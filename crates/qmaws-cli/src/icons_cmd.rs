//! Hidden development command `icons`: writes the program icon files from
//! the logo (`assets/icon/q-maws.svg`): PNG files in every size, the Windows
//! `q-maws.ico` and the macOS `q-maws.icns`. Run again whenever the logo
//! changes; the files are committed.

use qmaws_viz::icon;
use std::path::Path;
use std::process::ExitCode;

pub fn run(output: &Path) -> ExitCode {
    match write(output) {
        Ok(n) => {
            println!("{n} icon files written to {}.", output.display());
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("Error: {e}");
            ExitCode::from(1)
        }
    }
}

fn write(output: &Path) -> Result<usize, String> {
    let save = |name: String, data: &[u8]| -> Result<(), String> {
        std::fs::write(output.join(&name), data).map_err(|e| format!("{name}: {e}"))
    };
    std::fs::create_dir_all(output).map_err(|e| format!("{}: {e}", output.display()))?;
    let mut n = 0;
    for size in icon::PNG_SIZES {
        save(format!("q-maws-{size}.png"), &icon::png(size)?)?;
        n += 1;
    }
    let ico: Vec<(u32, Vec<u8>)> = icon::ICO_SIZES
        .iter()
        .map(|&s| icon::png(s).map(|p| (s, p)))
        .collect::<Result<_, _>>()?;
    save("q-maws.ico".into(), &icon::ico(&ico))?;
    let icns: Vec<(&[u8; 4], Vec<u8>)> = icon::ICNS_TYPES
        .iter()
        .map(|&(k, s)| icon::png(s).map(|p| (k, p)))
        .collect::<Result<_, _>>()?;
    save("q-maws.icns".into(), &icon::icns(&icns))?;
    Ok(n + 2)
}
