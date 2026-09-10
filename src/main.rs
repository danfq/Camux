mod platform;

use std::fmt::Write;

use platform::{CameraBackend, CameraDevice, PlatformBackend};
use unicode_width::UnicodeWidthStr;

fn main() {
    let backend = PlatformBackend::new();

    match backend.devices() {
        Ok(devices) => {
            if devices.is_empty() {
                println!("No camera devices found.");
            } else {
                println!("{}", devices_table(&devices));
            }
        }
        Err(error) => {
            eprintln!("Failed to enumerate cameras: {error}");
        }
    }
}

fn devices_table(devices: &[CameraDevice]) -> String {
    const HEADERS: [&str; 2] = ["DEVICE", "NAME"];

    let rows = devices
        .iter()
        .map(|device| [sanitize_cell(&device.id), sanitize_cell(&device.name)])
        .collect::<Vec<_>>();

    let mut widths = [display_width(HEADERS[0]), display_width(HEADERS[1])];
    for row in &rows {
        for (column, cell) in row.iter().enumerate() {
            widths[column] = widths[column].max(display_width(cell));
        }
    }

    let border = format!("+-{}-+-{}-+", "-".repeat(widths[0]), "-".repeat(widths[1]));
    let mut table = String::new();

    writeln!(table, "{border}").expect("writing to a String cannot fail");
    write_row(&mut table, &HEADERS, widths);
    writeln!(table, "{border}").expect("writing to a String cannot fail");
    for row in &rows {
        write_row(&mut table, &[&row[0], &row[1]], widths);
    }
    write!(table, "{border}").expect("writing to a String cannot fail");

    table
}

fn write_row(output: &mut String, cells: &[&str; 2], widths: [usize; 2]) {
    writeln!(
        output,
        "| {}{} | {}{} |",
        cells[0],
        " ".repeat(widths[0] - display_width(cells[0])),
        cells[1],
        " ".repeat(widths[1] - display_width(cells[1])),
    )
    .expect("writing to a String cannot fail");
}

fn sanitize_cell(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn display_width(value: &str) -> usize {
    UnicodeWidthStr::width(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_devices_as_an_aligned_table() {
        let table = devices_table(&[
            CameraDevice {
                id: "/dev/video0".to_owned(),
                name: "Integrated Camera".to_owned(),
            },
            CameraDevice {
                id: "/dev/video2".to_owned(),
                name: "USB Camera".to_owned(),
            },
        ]);

        assert_eq!(
            table,
            concat!(
                "+-------------+-------------------+\n",
                "| DEVICE      | NAME              |\n",
                "+-------------+-------------------+\n",
                "| /dev/video0 | Integrated Camera |\n",
                "| /dev/video2 | USB Camera        |\n",
                "+-------------+-------------------+",
            )
        );
    }

    #[test]
    fn sanitizes_multiline_values_and_aligns_wide_characters() {
        let table = devices_table(&[CameraDevice {
            id: "/dev/video0\nspoofed".to_owned(),
            name: "攝影機".to_owned(),
        }]);

        assert!(table.contains("| /dev/video0 spoofed | 攝影機 |"));
    }
}
