use erya::document::Document;
use erya::editor::Editor;
use erya::i18n::{Language, Message};
use erya::keymap::{DEFAULT_KEYMAP_TOML, KeyMap};
use erya::terminal;
use erya::workspace::Workspace;
use std::env;
use std::ffi::OsString;
use std::io;
use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("erya: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> io::Result<()> {
    let language = Language::detect();
    let Some(arguments) = parse_args(&language)? else {
        return Ok(());
    };
    let (keymap, keymap_warning) = KeyMap::load_with_fallback(arguments.keymap_path.as_deref());
    let documents = if arguments.document_paths.is_empty() {
        vec![Document::empty(None)]
    } else {
        arguments
            .document_paths
            .into_iter()
            .map(Document::open)
            .collect::<io::Result<Vec<_>>>()?
    };
    let warning_status = keymap_warning.map(|warning| language.keymap_load_failed(warning));
    let editors = documents
        .into_iter()
        .map(|document| {
            Editor::with_language_and_keymap(document, language.clone(), keymap.clone())
        })
        .collect();
    let mut workspace = Workspace::from_editors(editors);
    if let Some(status) = warning_status {
        workspace.set_status(status);
    }
    terminal::run(workspace)
}

struct Arguments {
    document_paths: Vec<PathBuf>,
    keymap_path: Option<PathBuf>,
}

fn parse_args(language: &Language) -> io::Result<Option<Arguments>> {
    let mut args = env::args_os().skip(1);
    let mut document_paths = Vec::new();
    let mut keymap_path = None;
    let mut options_done = false;

    while let Some(argument) = args.next() {
        if !options_done && argument == "--" {
            options_done = true;
            continue;
        }
        if !options_done && matches_option(&argument, &["-h", "--help"]) {
            print_help(language);
            return Ok(None);
        }
        if !options_done && matches_option(&argument, &["--languages"]) {
            print_languages(language);
            return Ok(None);
        }
        if !options_done && matches_option(&argument, &["--print-default-keymap"]) {
            print!("{DEFAULT_KEYMAP_TOML}");
            return Ok(None);
        }
        if !options_done && matches_option(&argument, &["--keymap"]) {
            let Some(path) = args.next() else {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    language.text(Message::KeymapPathRequired).to_owned(),
                ));
            };
            keymap_path = Some(PathBuf::from(path));
            continue;
        }
        if !options_done && matches_option(&argument, &["-V", "--version"]) {
            println!("erya {}", env!("CARGO_PKG_VERSION"));
            return Ok(None);
        }
        if !options_done && argument.to_string_lossy().starts_with('-') {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                language.unsupported_option(argument.to_string_lossy()),
            ));
        }
        document_paths.push(PathBuf::from(argument));
    }

    Ok(Some(Arguments {
        document_paths,
        keymap_path,
    }))
}

fn matches_option(argument: &OsString, candidates: &[&str]) -> bool {
    candidates.iter().any(|candidate| argument == candidate)
}

fn print_help(language: &Language) {
    println!("{}", language.text(Message::Help));
}

fn print_languages(language: &Language) {
    for available in language.available_languages() {
        println!("{}\t{}", available.code, available.name);
    }
}
