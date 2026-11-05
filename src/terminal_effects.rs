use std::io::{self, Write};

const BELL_CHUNK: [u8; 64] = [b'\x07'; 64];

pub(crate) fn write_terminal_bells<W: Write>(writer: &mut W, count: u16) -> io::Result<()> {
    let full_chunks = usize::from(count) / BELL_CHUNK.len();
    let remainder = usize::from(count) % BELL_CHUNK.len();
    for _ in 0..full_chunks {
        writer.write_all(&BELL_CHUNK)?;
    }
    writer.write_all(&BELL_CHUNK[..remainder])?;
    writer.flush()
}

pub(crate) fn write_window_title<W: Write>(writer: &mut W, title: Option<&str>) -> io::Result<()> {
    let title = title.unwrap_or("herdr");
    let safe_title = title
        .chars()
        .filter(|ch| !matches!(*ch, '\u{1b}' | '\u{7}' | '\u{9c}'))
        .collect::<String>();
    write!(writer, "\x1b]0;{safe_title}\x07")?;
    writer.flush()
}

/// Writes the aggregated pane progress as `OSC 9;4` for the outer terminal.
///
/// A determinate state without a value would read as 0% to the outer terminal,
/// so it is reported as indeterminate instead. tmux does not pass the sequence
/// through untranslated, so it is wrapped in the DCS passthrough there.
pub(crate) fn write_outer_progress<W: Write>(
    writer: &mut W,
    report: crate::protocol::endpoint::EndpointOuterProgress,
    tmux: bool,
) -> io::Result<()> {
    let Some(report) = report.validate() else {
        return Ok(());
    };
    let (state, progress) = match (report.state, report.progress) {
        (1, None) => (3, None),
        (state, progress) => (state, progress),
    };
    let mut sequence = match progress {
        Some(progress) => format!("\x1b]9;4;{state};{progress}\x1b\\").into_bytes(),
        None => format!("\x1b]9;4;{state}\x1b\\").into_bytes(),
    };
    if tmux {
        sequence = crate::terminal_notify::wrap_tmux_passthrough(&sequence);
    }
    writer.write_all(&sequence)?;
    writer.flush()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_exact_terminal_bell_count() {
        let mut output = Vec::new();

        write_terminal_bells(&mut output, 130).unwrap();

        assert_eq!(output, vec![b'\x07'; 130]);
    }

    #[test]
    fn window_title_strips_terminators_and_defaults_to_herdr() {
        let mut output = Vec::new();
        write_window_title(&mut output, Some("herdr\x1b api\u{7}\u{9c}")).unwrap();
        assert_eq!(output, b"\x1b]0;herdr api\x07");

        output.clear();
        write_window_title(&mut output, None).unwrap();
        assert_eq!(output, b"\x1b]0;herdr\x07");
    }

    fn progress(
        state: u8,
        progress: Option<u8>,
    ) -> crate::protocol::endpoint::EndpointOuterProgress {
        crate::protocol::endpoint::EndpointOuterProgress { state, progress }
    }

    #[test]
    fn outer_progress_writes_exact_osc_9_4_sequences() {
        let cases = [
            (progress(0, None), b"\x1b]9;4;0\x1b\\".as_slice()),
            (progress(1, Some(42)), b"\x1b]9;4;1;42\x1b\\".as_slice()),
            (progress(2, Some(7)), b"\x1b]9;4;2;7\x1b\\".as_slice()),
            (progress(2, None), b"\x1b]9;4;2\x1b\\".as_slice()),
            (progress(3, None), b"\x1b]9;4;3\x1b\\".as_slice()),
            (progress(4, Some(75)), b"\x1b]9;4;4;75\x1b\\".as_slice()),
            (progress(4, None), b"\x1b]9;4;4\x1b\\".as_slice()),
        ];
        for (report, expected) in cases {
            let mut output = Vec::new();
            write_outer_progress(&mut output, report, false).unwrap();
            assert_eq!(output, expected, "report {report:?}");
        }
    }

    #[test]
    fn determinate_progress_without_a_value_becomes_indeterminate() {
        let mut output = Vec::new();
        write_outer_progress(&mut output, progress(1, None), false).unwrap();
        assert_eq!(output, b"\x1b]9;4;3\x1b\\");
    }

    #[test]
    fn invalid_outer_progress_is_not_written() {
        for report in [progress(9, Some(10)), progress(1, Some(101))] {
            let mut output = Vec::new();
            write_outer_progress(&mut output, report, false).unwrap();
            assert!(output.is_empty(), "report {report:?}");
        }
    }

    #[test]
    fn tmux_outer_progress_is_wrapped_and_escapes_nested_escapes() {
        let mut output = Vec::new();
        write_outer_progress(&mut output, progress(1, Some(42)), true).unwrap();
        assert_eq!(output, b"\x1bPtmux;\x1b\x1b]9;4;1;42\x1b\x1b\\\x1b\\");
    }
}
