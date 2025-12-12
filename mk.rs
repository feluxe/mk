use std::env;
use std::fs::File;
use std::fs::OpenOptions;
use std::io::prelude::*;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process;
use std::process::Command;

fn ensure_make_py_exists(make_py_file: String) {
    if Path::new(&make_py_file).exists() == false {
        eprintln!("mk: Cannot find 'make.py' file.");
        process::exit(1);
    }
}

// Function to get venv path using 'uv'
fn get_venv_path_from_uv() -> Option<String> {
    let output = Command::new("uv")
        .arg("run")
        .arg("python")
        .arg("-c")
        .arg("import os; print(os.environ['VIRTUAL_ENV'])")
        .output();

    // If uv command fails (e.g., uv is not installed), return None to fall back to poetry
    let result = match output {
        Ok(out) => out,
        Err(_) => return None,
    };

    if !result.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&result.stdout);
    let venv_path = stdout.trim().to_string();

    if venv_path.is_empty() {
        return None;
    }

    Some(venv_path)
}

fn get_venv_path_from_poetry() -> String {
    let result = Command::new("poetry")
        .arg("env")
        .arg("info")
        .arg("--path")
        .output()
        .expect("mk: Failed to execute 'poetry env info --path'");

    if !result.status.success() {
        let msg1 = format!(
            "mk: Command 'poetry env info --path' returned {}\n\n",
            result.status
        );
        let msg2 = "This usually means there is no venv.";
        eprintln!("{}{}", msg1, msg2);
        process::exit(1);
    }

    let venv_path = String::from_utf8_lossy(&result.stdout).trim().to_string();

    if venv_path.is_empty() {
        eprintln!("mk: No venv found for current working directory.");
        process::exit(1);
    }

    return venv_path;
}

fn get_venv_path(cur_dir: String, cache_file: String) -> String {
    let f_result = File::open(&cache_file);

    let mut venv_path: std::string::String = "".to_string();

    if let Ok(f) = f_result {
        let f = BufReader::new(f);

        // Try reading env path from cache.
        for line in f.lines() {
            let line = line.expect("mk: Unable to read line");

            let cur_dir_with_space = format!("{} ", cur_dir);

            if line.starts_with(&cur_dir_with_space) {
                venv_path = line.clone();
                let v: Vec<&str> = venv_path.split_whitespace().collect();
                venv_path = v.get(1).unwrap().trim().to_string();
            }
        }

        // If a venv path exists in cache, check if python bin can be found.
        if !venv_path.is_empty() {
            let python_bin = format!("{}/bin/python", venv_path);

            if !Path::new(&python_bin).exists() {
                // If the path in the cache is bad, clear it and force a re-check via the tools below.
                venv_path = "".to_string();
            }
        }
    }

    // If venv path cannot be found in cache, try 'uv', then 'poetry'.
    if venv_path.is_empty() {
        // Try 'uv' first
        if let Some(path) = get_venv_path_from_uv() {
            venv_path = path;
        } else {
            // Fallback to 'poetry'
            venv_path = get_venv_path_from_poetry();
        }

        // Write the newly found path to the cache file (create if necessary)
        if let Ok(mut file) = OpenOptions::new()
            .write(true)
            .append(true)
            .create(true) // create file if it doesn't exist
            .open(&cache_file)
        {
            let new_line = format!("{} {}", cur_dir, venv_path);

            if let Err(e) = writeln!(file, "{}", new_line) {
                eprintln!("mk: Couldn't write to file: {}", e);
            }
        } else {
            eprintln!(
                "mk: Couldn't open or create cache file for writing: {}",
                cache_file
            );
            process::exit(1);
        }
    }

    return venv_path;
}

fn main() {
    //
    let cur_dir_path = env::current_dir().expect("mk: Cannot read the current dir.");
    let cur_dir = cur_dir_path.as_path().display().to_string();
    let home_dir = env::home_dir().expect("mk: Cannot read home dir.");
    // Ensure cache directory exists before trying to open the file
    let cache_dir = format!("{}/.cache/mewo_mk", home_dir.display());
    std::fs::create_dir_all(&cache_dir).expect("mk: Failed to create cache directory");

    let cache_file = format!("{}/cache", cache_dir);
    let make_py_file = format!("{}/{}", cur_dir, "make.py");

    ensure_make_py_exists(make_py_file.clone());

    let venv_path = get_venv_path(cur_dir.clone(), cache_file.clone());

    // Pass caller args to our command.
    let mut args_raw: Vec<String> = env::args().collect();
    let args = args_raw.drain(1..);

    // We need to add the virtualenv bin/ directory to PATH of the script.
    // This ensures that when 'python' is called from within the script it uses
    // the interpreter from the virtualenv.
    let proc_env_path: String = env::var("PATH").expect("mk: Cannot read PATH from environment.");
    let python_bin_dir = format!("{}/bin", venv_path);
    let updated_proc_env_path = format!("{}:{}", python_bin_dir, proc_env_path);

    let python_bin = format!("{}/bin/python", venv_path);

    Command::new(python_bin.clone())
        .arg("make.py")
        .args(args)
        .env("PATH", updated_proc_env_path.clone())
        .status()
        .expect("mk: failed to execute process");
}
