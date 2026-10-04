use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

pub(crate) const TAB_WIDTH: usize = 4;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct GraphemeCell {
    pub(crate) start_char: usize,
    pub(crate) end_char: usize,
    pub(crate) start_display: usize,
    pub(crate) end_display: usize,
    pub(crate) text: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct VisualRow {
    pub(crate) document_row: usize,
    pub(crate) start_display: usize,
    pub(crate) end_display: usize,
}

pub(crate) fn grapheme_cells(line: &[char]) -> Vec<GraphemeCell> {
    let text: String = line.iter().collect();
    let mut char_col = 0;
    let mut display_col = 0;
    text.graphemes(true)
        .map(|grapheme| {
            let char_len = grapheme.chars().count();
            let width = if grapheme == "\t" {
                TAB_WIDTH - display_col % TAB_WIDTH
            } else if grapheme.chars().any(char::is_control) {
                UnicodeWidthStr::width_cjk("�")
            } else {
                UnicodeWidthStr::width_cjk(grapheme)
            };
            let cell = GraphemeCell {
                start_char: char_col,
                end_char: char_col + char_len,
                start_display: display_col,
                end_display: display_col + width,
                text: grapheme.to_owned(),
            };
            char_col = cell.end_char;
            display_col = cell.end_display;
            cell
        })
        .collect()
}

pub(crate) fn display_col(line: &[char], char_col: usize) -> usize {
    grapheme_cells(line)
        .into_iter()
        .take_while(|cell| cell.end_char <= char_col)
        .last()
        .map_or(0, |cell| cell.end_display)
}

pub(crate) fn line_display_width(line: &[char]) -> usize {
    grapheme_cells(line)
        .last()
        .map_or(0, |cell| cell.end_display)
}

pub(crate) fn wrapped_visual_rows(
    document_row: usize,
    line: &[char],
    width: usize,
) -> Vec<VisualRow> {
    let width = width.max(1);
    let cells = grapheme_cells(line);
    let line_width = cells.last().map_or(0, |cell| cell.end_display);
    if cells.is_empty() {
        return vec![VisualRow {
            document_row,
            start_display: 0,
            end_display: 0,
        }];
    }

    let mut rows = Vec::new();
    let mut start_display = 0;
    for cell in cells {
        if cell.end_display.saturating_sub(start_display) > width
            && cell.start_display > start_display
        {
            rows.push(VisualRow {
                document_row,
                start_display,
                end_display: cell.start_display,
            });
            start_display = cell.start_display;
        }
    }
    rows.push(VisualRow {
        document_row,
        start_display,
        end_display: line_width,
    });
    if line_width.saturating_sub(start_display) == width {
        rows.push(VisualRow {
            document_row,
            start_display: line_width,
            end_display: line_width,
        });
    }
    rows
}

pub(crate) fn previous_grapheme_boundary(line: &[char], char_col: usize) -> usize {
    grapheme_cells(line)
        .into_iter()
        .take_while(|cell| cell.end_char < char_col)
        .last()
        .map_or(0, |cell| cell.end_char)
}

pub(crate) fn next_grapheme_boundary(line: &[char], char_col: usize) -> usize {
    grapheme_cells(line)
        .into_iter()
        .find(|cell| cell.end_char > char_col)
        .map_or(line.len(), |cell| cell.end_char)
}

pub(crate) fn closest_char_col_for_display(line: &[char], target: usize) -> usize {
    let mut closest_col = 0;
    let mut closest_distance = target;
    for cell in grapheme_cells(line) {
        let distance = cell.end_display.abs_diff(target);
        if distance < closest_distance {
            closest_col = cell.end_char;
            closest_distance = distance;
        }
    }
    closest_col
}

pub(crate) fn character_range_for_display_columns(
    line: &[char],
    left: usize,
    right: usize,
) -> Option<(usize, usize)> {
    let mut range = None;
    for cell in grapheme_cells(line) {
        if cell.start_display < right && cell.end_display > left {
            range.get_or_insert((cell.start_char, cell.end_char)).1 = cell.end_char;
        }
        if cell.end_display >= right {
            break;
        }
    }
    range
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn treats_combining_and_zwj_sequences_as_single_cells() {
        let line: Vec<char> = "e\u{301}👩‍💻x".chars().collect();
        let cells = grapheme_cells(&line);

        assert_eq!(cells.len(), 3);
        assert_eq!((cells[0].start_char, cells[0].end_char), (0, 2));
        assert_eq!((cells[1].start_char, cells[1].end_char), (2, 5));
        assert_eq!(next_grapheme_boundary(&line, 0), 2);
        assert_eq!(next_grapheme_boundary(&line, 2), 5);
        assert_eq!(previous_grapheme_boundary(&line, 5), 2);
    }

    #[test]
    fn wraps_only_at_grapheme_boundaries_and_keeps_exact_width_cursor_row() {
        let line: Vec<char> = "ab👩‍💻cd".chars().collect();

        assert_eq!(
            wrapped_visual_rows(7, &line, 4),
            vec![
                VisualRow {
                    document_row: 7,
                    start_display: 0,
                    end_display: 4,
                },
                VisualRow {
                    document_row: 7,
                    start_display: 4,
                    end_display: 6,
                },
            ]
        );
        assert_eq!(
            wrapped_visual_rows(0, &"abcd".chars().collect::<Vec<_>>(), 4).len(),
            2
        );
    }
}
