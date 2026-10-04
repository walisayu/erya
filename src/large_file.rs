use std::fs::{self, File, Metadata};
use std::io::{self, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::SystemTime;

const INDEX_BUFFER_SIZE: usize = 1024 * 1024;
const INDEX_STRIDE: usize = 4096;
const MAX_LINE_BYTES: usize = 1024 * 1024;
const APPEND_GUARD_BYTES: usize = 4096;
const UTF8_BOM: &[u8] = b"\xEF\xBB\xBF";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct LineCheckpoint {
    row: usize,
    start: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct FileIdentity {
    #[cfg(unix)]
    device: u64,
    #[cfg(unix)]
    inode: u64,
    #[cfg(not(unix))]
    created: Option<SystemTime>,
}

#[derive(Debug)]
struct ScanResult {
    checkpoints: Vec<LineCheckpoint>,
    line_count: usize,
    last_line_start: u64,
    scanned_len: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Refresh {
    Unchanged,
    Appended,
    Reloaded,
}

#[derive(Debug)]
pub(crate) struct LargeFileBuffer {
    path: PathBuf,
    file: Mutex<File>,
    checkpoints: Vec<LineCheckpoint>,
    line_count: usize,
    last_line_start: u64,
    file_len: u64,
    modified: Option<SystemTime>,
    identity: FileIdentity,
    head_guard: Vec<u8>,
    tail_guard: Vec<u8>,
    has_bom: bool,
}

impl LargeFileBuffer {
    pub(crate) fn open(path: &Path) -> io::Result<Self> {
        let mut file = File::open(path)?;
        let has_bom = detect_bom(&mut file)?;
        let first_line_start = bom_len(has_bom);
        let scan = build_line_index(&mut file, first_line_start, 0)?;
        let metadata = file.metadata()?;
        let (head_guard, tail_guard) = read_append_guards(&mut file, scan.scanned_len)?;
        Ok(Self {
            path: path.to_path_buf(),
            file: Mutex::new(file),
            checkpoints: scan.checkpoints,
            line_count: scan.line_count,
            last_line_start: scan.last_line_start,
            file_len: scan.scanned_len,
            modified: metadata.modified().ok(),
            identity: file_identity(&metadata),
            head_guard,
            tail_guard,
            has_bom,
        })
    }

    pub(crate) fn line_count(&self) -> usize {
        self.line_count
    }

    pub(crate) fn line(&self, row: usize) -> Vec<char> {
        if row >= self.line_count {
            return Vec::new();
        }
        let checkpoint = self.checkpoint_before_row(row);
        let Ok(mut file) = self.file.lock() else {
            return Vec::new();
        };
        read_line(&mut file, checkpoint, row).unwrap_or_default()
    }

    pub(crate) fn line_len(&self, row: usize) -> usize {
        self.line(row).len()
    }

    pub(crate) fn file_len(&self) -> u64 {
        self.file_len
    }

    pub(crate) fn has_bom(&self) -> bool {
        self.has_bom
    }

    pub(crate) fn read_bytes(&self, offset: u64, len: usize) -> Vec<u8> {
        if offset >= self.file_len || len == 0 {
            return Vec::new();
        }
        let available = (self.file_len - offset).min(len as u64) as usize;
        let Ok(mut file) = self.file.lock() else {
            return Vec::new();
        };
        if file.seek(SeekFrom::Start(offset)).is_err() {
            return Vec::new();
        }
        let mut buffer = vec![0; available];
        match file.read_exact(&mut buffer) {
            Ok(()) => buffer,
            Err(_) => Vec::new(),
        }
    }

    pub(crate) fn line_start(&self, row: usize) -> u64 {
        let checkpoint = self.checkpoint_before_row(row);
        let Ok(mut file) = self.file.lock() else {
            return checkpoint.start;
        };
        scan_line_start(&mut file, checkpoint, row).unwrap_or(checkpoint.start)
    }

    pub(crate) fn row_at_byte(&self, offset: u64) -> usize {
        let checkpoint = self.checkpoint_before_offset(offset);
        let Ok(mut file) = self.file.lock() else {
            return checkpoint.row;
        };
        scan_row_at_byte(&mut file, checkpoint, offset)
            .unwrap_or(checkpoint.row)
            .min(self.line_count.saturating_sub(1))
    }

    fn checkpoint_before_row(&self, row: usize) -> LineCheckpoint {
        self.checkpoints
            .partition_point(|checkpoint| checkpoint.row <= row)
            .checked_sub(1)
            .and_then(|index| self.checkpoints.get(index))
            .copied()
            .unwrap_or(LineCheckpoint {
                row: 0,
                start: bom_len(self.has_bom),
            })
    }

    fn checkpoint_before_offset(&self, offset: u64) -> LineCheckpoint {
        self.checkpoints
            .partition_point(|checkpoint| checkpoint.start <= offset)
            .checked_sub(1)
            .and_then(|index| self.checkpoints.get(index))
            .copied()
            .unwrap_or(LineCheckpoint {
                row: 0,
                start: bom_len(self.has_bom),
            })
    }

    pub(crate) fn refresh(&mut self) -> io::Result<Refresh> {
        let path_metadata = fs::metadata(&self.path)?;
        let modified = path_metadata.modified().ok();
        let identity = file_identity(&path_metadata);
        if identity == self.identity
            && path_metadata.len() == self.file_len
            && modified == self.modified
        {
            return Ok(Refresh::Unchanged);
        }

        let mut file = File::open(&self.path)?;
        let has_bom = detect_bom(&mut file)?;
        let appended = identity == self.identity
            && path_metadata.len() > self.file_len
            && has_bom == self.has_bom
            && append_guards_match(&mut file, self.file_len, &self.head_guard, &self.tail_guard)?;

        let scan = if appended {
            let last_row = self.line_count.saturating_sub(1);
            let mut scan = build_line_index(&mut file, self.last_line_start, last_row)?;
            self.checkpoints
                .retain(|checkpoint| checkpoint.row < last_row);
            self.checkpoints.append(&mut scan.checkpoints);
            scan
        } else {
            build_line_index(&mut file, bom_len(has_bom), 0)?
        };
        let metadata = file.metadata()?;
        let (head_guard, tail_guard) = read_append_guards(&mut file, scan.scanned_len)?;

        if !appended {
            self.checkpoints = scan.checkpoints;
        }
        self.file = Mutex::new(file);
        self.line_count = scan.line_count;
        self.last_line_start = scan.last_line_start;
        self.file_len = scan.scanned_len;
        self.modified = metadata.modified().ok();
        self.identity = file_identity(&metadata);
        self.head_guard = head_guard;
        self.tail_guard = tail_guard;
        self.has_bom = has_bom;
        Ok(if appended {
            Refresh::Appended
        } else {
            Refresh::Reloaded
        })
    }
}

fn bom_len(has_bom: bool) -> u64 {
    if has_bom { UTF8_BOM.len() as u64 } else { 0 }
}

fn detect_bom(file: &mut File) -> io::Result<bool> {
    file.seek(SeekFrom::Start(0))?;
    let mut prefix = [0; 3];
    let read = file.read(&mut prefix)?;
    Ok(read == prefix.len() && prefix == UTF8_BOM)
}

fn build_line_index(file: &mut File, start: u64, start_row: usize) -> io::Result<ScanResult> {
    file.seek(SeekFrom::Start(start))?;
    let mut checkpoints = Vec::new();
    if start_row % INDEX_STRIDE == 0 {
        checkpoints.push(LineCheckpoint {
            row: start_row,
            start,
        });
    }
    let mut buffer = vec![0; INDEX_BUFFER_SIZE];
    let mut validation = Vec::with_capacity(INDEX_BUFFER_SIZE + 4);
    let mut carry = Vec::with_capacity(4);
    let mut absolute = start;
    let mut row = start_row;
    let mut line_start = start;
    let mut pending_cr = false;

    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        validate_utf8_chunk(&buffer[..read], &mut validation, &mut carry)?;
        for byte in &buffer[..read] {
            if pending_cr {
                if *byte == b'\n' {
                    advance_line(&mut row, &mut line_start, absolute + 1, &mut checkpoints);
                    pending_cr = false;
                    absolute += 1;
                    continue;
                }
                advance_line(&mut row, &mut line_start, absolute, &mut checkpoints);
                pending_cr = false;
            }

            match *byte {
                b'\r' => pending_cr = true,
                b'\n' => advance_line(&mut row, &mut line_start, absolute + 1, &mut checkpoints),
                _ => {}
            }
            absolute += 1;
        }
    }
    if !carry.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "incomplete UTF-8 sequence at end of file",
        ));
    }
    if pending_cr {
        advance_line(&mut row, &mut line_start, absolute, &mut checkpoints);
    }
    Ok(ScanResult {
        checkpoints,
        line_count: row + 1,
        last_line_start: line_start,
        scanned_len: absolute,
    })
}

fn advance_line(
    row: &mut usize,
    line_start: &mut u64,
    next_start: u64,
    checkpoints: &mut Vec<LineCheckpoint>,
) {
    *row += 1;
    *line_start = next_start;
    if *row % INDEX_STRIDE == 0 {
        checkpoints.push(LineCheckpoint {
            row: *row,
            start: next_start,
        });
    }
}

fn read_line(
    file: &mut File,
    checkpoint: LineCheckpoint,
    target_row: usize,
) -> io::Result<Vec<char>> {
    file.seek(SeekFrom::Start(checkpoint.start))?;
    let mut buffer = vec![0; 64 * 1024];
    let mut bytes = Vec::new();
    let mut row = checkpoint.row;
    let mut pending_cr = false;

    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        for byte in &buffer[..read] {
            if pending_cr {
                if *byte == b'\n' {
                    row += 1;
                    pending_cr = false;
                    continue;
                }
                row += 1;
                pending_cr = false;
            }

            match *byte {
                b'\r' if row == target_row => return Ok(bytes_to_chars(bytes)),
                b'\r' => pending_cr = true,
                b'\n' if row == target_row => return Ok(bytes_to_chars(bytes)),
                b'\n' => row += 1,
                byte if row == target_row => {
                    bytes.push(byte);
                    if bytes.len() == MAX_LINE_BYTES {
                        return Ok(bytes_to_chars(bytes));
                    }
                }
                _ => {}
            }
        }
    }
    if pending_cr {
        row += 1;
    }
    if row == target_row {
        Ok(bytes_to_chars(bytes))
    } else {
        Ok(Vec::new())
    }
}

fn scan_line_start(
    file: &mut File,
    checkpoint: LineCheckpoint,
    target_row: usize,
) -> io::Result<u64> {
    if checkpoint.row >= target_row {
        return Ok(checkpoint.start);
    }
    file.seek(SeekFrom::Start(checkpoint.start))?;
    let mut buffer = vec![0; 64 * 1024];
    let mut row = checkpoint.row;
    let mut absolute = checkpoint.start;
    let mut pending_cr = false;

    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        for byte in &buffer[..read] {
            if pending_cr {
                pending_cr = false;
                if *byte == b'\n' {
                    row += 1;
                    absolute += 1;
                    if row == target_row {
                        return Ok(absolute);
                    }
                    continue;
                }
                row += 1;
                if row == target_row {
                    return Ok(absolute);
                }
            }

            match *byte {
                b'\r' => pending_cr = true,
                b'\n' => {
                    row += 1;
                    if row == target_row {
                        return Ok(absolute + 1);
                    }
                }
                _ => {}
            }
            absolute += 1;
        }
    }
    Ok(absolute)
}

fn scan_row_at_byte(file: &mut File, checkpoint: LineCheckpoint, offset: u64) -> io::Result<usize> {
    if offset <= checkpoint.start {
        return Ok(checkpoint.row);
    }
    file.seek(SeekFrom::Start(checkpoint.start))?;
    let mut buffer = vec![0; 64 * 1024];
    let mut row = checkpoint.row;
    let mut absolute = checkpoint.start;
    let mut pending_cr = false;

    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        for byte in &buffer[..read] {
            if pending_cr {
                pending_cr = false;
                row += 1;
                if *byte == b'\n' {
                    if absolute >= offset {
                        return Ok(row.saturating_sub(1));
                    }
                    absolute += 1;
                    continue;
                }
            }
            if absolute >= offset {
                return Ok(row);
            }

            match *byte {
                b'\r' => pending_cr = true,
                b'\n' => row += 1,
                _ => {}
            }
            absolute += 1;
        }
    }
    Ok(row)
}

fn bytes_to_chars(mut bytes: Vec<u8>) -> Vec<char> {
    while std::str::from_utf8(&bytes).is_err() {
        bytes.pop();
    }
    String::from_utf8(bytes)
        .map(|text| text.chars().collect())
        .unwrap_or_default()
}

fn validate_utf8_chunk(
    chunk: &[u8],
    validation: &mut Vec<u8>,
    carry: &mut Vec<u8>,
) -> io::Result<()> {
    validation.clear();
    validation.extend_from_slice(carry);
    validation.extend_from_slice(chunk);
    carry.clear();
    match std::str::from_utf8(validation) {
        Ok(_) => Ok(()),
        Err(error) if error.error_len().is_none() => {
            carry.extend_from_slice(&validation[error.valid_up_to()..]);
            Ok(())
        }
        Err(error) => Err(io::Error::new(io::ErrorKind::InvalidData, error)),
    }
}

fn read_append_guards(file: &mut File, file_len: u64) -> io::Result<(Vec<u8>, Vec<u8>)> {
    let guard_len = file_len.min(APPEND_GUARD_BYTES as u64) as usize;
    let mut head = vec![0; guard_len];
    file.seek(SeekFrom::Start(0))?;
    file.read_exact(&mut head)?;
    let mut tail = vec![0; guard_len];
    file.seek(SeekFrom::Start(file_len - guard_len as u64))?;
    file.read_exact(&mut tail)?;
    Ok((head, tail))
}

fn append_guards_match(
    file: &mut File,
    previous_len: u64,
    expected_head: &[u8],
    expected_tail: &[u8],
) -> io::Result<bool> {
    let (head, tail) = read_append_guards(file, previous_len)?;
    Ok(head == expected_head && tail == expected_tail)
}

#[cfg(unix)]
fn file_identity(metadata: &Metadata) -> FileIdentity {
    use std::os::unix::fs::MetadataExt;

    FileIdentity {
        device: metadata.dev(),
        inode: metadata.ino(),
    }
}

#[cfg(not(unix))]
fn file_identity(metadata: &Metadata) -> FileIdentity {
    FileIdentity {
        created: metadata.created().ok(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn test_path(name: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        std::env::temp_dir().join(format!("erya-{name}-{unique}.txt"))
    }

    #[test]
    fn sparse_index_reads_lines_across_checkpoints() {
        let path = test_path("sparse-index");
        let mut writer = File::create(&path).expect("create indexed file");
        for row in 0..(INDEX_STRIDE + 3) {
            writeln!(writer, "第{row}行").expect("write indexed line");
        }
        writer.flush().expect("flush indexed file");

        let buffer = LargeFileBuffer::open(&path).expect("open indexed file");

        assert_eq!(buffer.line_count(), INDEX_STRIDE + 4);
        assert_eq!(
            buffer.line(INDEX_STRIDE + 1).iter().collect::<String>(),
            format!("第{}行", INDEX_STRIDE + 1)
        );
        assert!(buffer.checkpoints.len() < buffer.line_count() / 100);

        fs::remove_file(path).expect("remove indexed file");
    }

    #[test]
    fn line_reads_are_bounded_for_a_single_huge_line() {
        let path = test_path("bounded-line");
        let mut writer = File::create(&path).expect("create long-line file");
        writer
            .write_all(&vec![b'x'; MAX_LINE_BYTES + 1024])
            .expect("write long line");
        writer.flush().expect("flush long-line file");

        let buffer = LargeFileBuffer::open(&path).expect("open long-line file");

        assert_eq!(buffer.line_count(), 1);
        assert_eq!(buffer.line(0).len(), MAX_LINE_BYTES);

        fs::remove_file(path).expect("remove long-line file");
    }

    #[test]
    fn opens_a_sparse_500_mib_file_with_bounded_memory_structures() {
        const FILE_SIZE: u64 = 500 * 1024 * 1024;
        let path = test_path("500-mib");
        let file = File::create(&path).expect("create 500 MiB file");
        file.set_len(FILE_SIZE).expect("size 500 MiB sparse file");
        drop(file);

        let buffer = LargeFileBuffer::open(&path).expect("index 500 MiB file");

        assert_eq!(buffer.file_len(), FILE_SIZE);
        assert_eq!(buffer.line_count(), 1);
        assert_eq!(buffer.checkpoints.len(), 1);
        assert_eq!(buffer.line(0).len(), MAX_LINE_BYTES);

        fs::remove_file(path).expect("remove 500 MiB file");
    }
}
