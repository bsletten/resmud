//! Play the sample realm at a prompt, through the first door: `cargo run --example door`.
//!
//! ```text
//! cargo run --example door                    # alice, in a scratch copy of realm/first
//! cargo run --example door -- <dir> <name>    # the character <name>, in the realm at <dir>
//! ```
//!
//! With no directory, the realm is copied into a scratch directory first, so playing never
//! changes the repository's `realm/first`; give a directory to play a realm in place (every
//! move writes its files). Type `look`, `help`, `go east`, `take lamp`, and `quit` to leave.
//! Running a verb a player wrote needs one published and signed by its author, which this
//! example does not do: it holds no author's key.

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;

use resmud::door::{serve, Door};
use resmud::Realm;

fn copy_dir(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let given = args.next().map(PathBuf::from);
    let character = args.next().unwrap_or_else(|| "alice".to_string());
    let scratch;
    let root = match given {
        Some(dir) => dir,
        None => {
            scratch = match tempfile::tempdir() {
                Ok(dir) => dir,
                Err(e) => {
                    eprintln!("Could not make a scratch directory: {e}.");
                    return ExitCode::FAILURE;
                }
            };
            let root = scratch.path().join("first");
            let sample = Path::new(env!("CARGO_MANIFEST_DIR")).join("realm/first");
            if let Err(e) = copy_dir(&sample, &root) {
                eprintln!("Could not copy the sample realm: {e}.");
                return ExitCode::FAILURE;
            }
            root
        }
    };
    let realm = match Realm::new("first", &root) {
        Ok(realm) => realm,
        Err(e) => {
            eprintln!("{}", resmud::refusal(&e));
            return ExitCode::FAILURE;
        }
    };
    let kernel = match resmud::kernel(&realm, Vec::new()) {
        Ok(kernel) => Arc::new(kernel),
        Err(e) => {
            eprintln!("The realm could not be opened: {e}.");
            return ExitCode::FAILURE;
        }
    };
    let door = Door::new(kernel, &realm, &character);
    println!("Type look to look around, help to see what you can do here, and quit to leave.");
    match serve(&door, std::io::stdin().lock(), std::io::stdout().lock()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("The door closed: {e}.");
            ExitCode::FAILURE
        }
    }
}
