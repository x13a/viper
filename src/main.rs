use std::collections::HashSet;
use std::env;
use std::error;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::exit;
use std::result;

use rand::rngs::ThreadRng;
use rand::seq::SliceRandom;

const EXIT_SUCCESS: i32 = 0;
const EXIT_USAGE: i32 = 2;

mod flag {
    pub const HELP: &'static str = "h";
    pub const VERSION: &'static str = "V";
    pub const VERBOSE: &'static str = "v";
    pub const RECURSIVE: &'static str = "r";
    pub const ZERO: &'static str = "z";
    pub const NUM_ROUNDS: &'static str = "n";
    pub const BLOCK_SIZE: &'static str = "b";
}

mod default {
    pub const NUM_ROUNDS: i32 = 1;
    pub const BLOCK_SIZE: i32 = 8;
}

enum PrintDestination {
    Stdout,
    Stderr,
}

fn print_usage(to: PrintDestination) {
    let usage = format!(
        "{P} [-{h}|{V}] [-{v}{v}] [-{r}] [-{z}] [-{n} NUM] [-{b} NUM] FILES\n\n\
         [-{h}] * Print help and exit\n\
         [-{V}] * Print version and exit\n\
         [-{v}] * Tell what is going on\n\
         [-{r}] * Walk directories recursively\n\
         [-{z}] * First overwrite with zeroes\n\
         [-{n}] * Number of rounds to overwrite (default: {dn})\n\
         [-{b}] * Maximum block size in MB (default: {db})",
        P = PathBuf::from(env::args_os().next().unwrap())
            .file_name()
            .unwrap()
            .to_string_lossy(),
        h = flag::HELP,
        V = flag::VERSION,
        v = flag::VERBOSE,
        r = flag::RECURSIVE,
        z = flag::ZERO,
        n = flag::NUM_ROUNDS,
        b = flag::BLOCK_SIZE,
        dn = default::NUM_ROUNDS,
        db = default::BLOCK_SIZE,
    );
    match to {
        PrintDestination::Stdout => println!("{}", usage),
        PrintDestination::Stderr => eprintln!("{}", usage),
    }
}

type Result<T> = result::Result<T, Box<dyn error::Error>>;

#[derive(Default)]
struct Opts {
    verbose: i32,
    recursive: bool,
    zero: bool,
    num_rounds: i32,
    block_size: i32,
    files: HashSet<PathBuf>,
}

fn get_opts() -> Result<Opts> {
    let mut argv = env::args_os().skip(1);
    if argv.len() == 0 {
        print_usage(PrintDestination::Stderr);
        exit(EXIT_USAGE);
    }
    let mut opts = Opts::default();
    let missing_arg = |s| {
        eprintln!("missing value for: -{}", s);
        exit(EXIT_USAGE);
    };
    loop {
        let arg = match argv.next() {
            Some(v) => match v.into_string() {
                Ok(s) => s,
                Err(v) => {
                    opts.files.insert(v.into());
                    continue;
                }
            },
            None => break,
        };
        if !arg.starts_with('-') {
            if !arg.is_empty() {
                opts.files.insert(arg.into());
            }
            continue;
        }
        for c in arg.chars().skip(1) {
            match c.to_string().as_str() {
                flag::HELP => {
                    print_usage(PrintDestination::Stdout);
                    exit(EXIT_SUCCESS);
                }
                flag::VERSION => {
                    println!("{}", env!("CARGO_PKG_VERSION"));
                    exit(EXIT_SUCCESS);
                }
                flag::VERBOSE => opts.verbose += 1,
                flag::RECURSIVE => opts.recursive = true,
                flag::ZERO => opts.zero = true,
                flag::NUM_ROUNDS => match argv.next() {
                    Some(s) => opts.num_rounds = s.to_str().unwrap().parse()?,
                    None => missing_arg(flag::NUM_ROUNDS),
                },
                flag::BLOCK_SIZE => match argv.next() {
                    Some(s) => opts.block_size = s.to_str().unwrap().parse()?,
                    None => missing_arg(flag::BLOCK_SIZE),
                },
                _ => {}
            }
        }
    }
    if opts.files.len() == 0 {
        eprintln!("no files");
        exit(EXIT_USAGE);
    }
    if opts.num_rounds < 1 {
        opts.num_rounds = default::NUM_ROUNDS;
    }
    if opts.block_size < 1 {
        opts.block_size = default::BLOCK_SIZE;
    }
    opts.block_size *= 1 << 20;
    Ok(opts)
}

fn make_values() -> Vec<String> {
    let mut res = Vec::with_capacity(62);
    for a in 0..2 {
        for b in 1..if a != 0 { 8 } else { 9 } {
            for c in 0..4 {
                let mut s = String::with_capacity(14);
                s.push('8');
                if a != 0 {
                    s.push('#');
                }
                s.push_str("=".repeat(b).as_str());
                s.push('D');
                if c != 0 {
                    s.push(' ');
                    s.push_str("~".repeat(c).as_str());
                }
                res.push(s);
            }
        }
    }
    res.push("{()}".into());
    res.push("({})".into());
    res
}

fn make_block(size: u64, params: &mut Params) -> Vec<u8> {
    let mut res = Vec::with_capacity(size as usize);
    let mut pos = 0;
    while pos < size {
        let value = params.values.choose(&mut params.rng).unwrap().as_bytes();
        let value_len = value.len() as u64;
        pos += value_len + 1;
        if pos > size {
            res.extend_from_slice(&value[..(value_len - (pos - size) - 1) as usize]);
            break;
        }
        res.extend_from_slice(value);
        res.push(b' ');
    }
    res
}

fn wipe(path: &Path, round: i32, params: &mut Params) -> Result<()> {
    let mut file = fs::OpenOptions::new().write(true).open(path)?;
    let file_size = file.metadata()?.len();
    if file_size == 0 {
        return Err(format!("file: {} size is zero", path.display()).into());
    }
    let block_size;
    let tmp;
    let block: &[u8] = if round == 0 {
        block_size = params.opts.block_size as u64;
        params.zero_block.as_ref().unwrap()
    } else {
        block_size = file_size.min(params.opts.block_size as u64);
        tmp = make_block(block_size, params);
        &tmp
    };
    let mut pos = 0;
    while pos < file_size {
        let mut value = block;
        pos += block_size;
        if pos > file_size {
            value = &value[..(block_size - (pos - file_size)) as usize];
        }
        file.write_all(value)?;
    }
    file.sync_all()?;
    Ok(())
}

fn wipe_loop(path: &Path, params: &mut Params) -> Result<()> {
    if params.opts.verbose == 1 {
        println!("[wipe] {}", path.display());
    }
    for n in 0..params.opts.num_rounds + 1 {
        if n == 0 && !params.opts.zero {
            continue;
        }
        if params.opts.verbose > 1 {
            println!("[round: {}] {}", n, path.display());
        }
        wipe(path, n, params)?;
    }
    let new_path = &path.with_file_name(params.values.choose(&mut params.rng).unwrap());
    fs::rename(path, new_path)?;
    fs::remove_file(new_path)?;
    Ok(())
}

fn walk(path: &Path, depth: i32, params: &mut Params) -> Result<()> {
    let path = &path.canonicalize()?;
    if fs::metadata(path)?.is_dir() {
        if depth > 0 && !params.opts.recursive {
            return Ok(());
        }
        for entry in fs::read_dir(path)? {
            let entry = match entry {
                Ok(v) => v,
                Err(err) => {
                    params.error_counter += 1;
                    eprintln!("{}", err);
                    continue;
                }
            };
            walk1(&entry.path(), depth + 1, params);
        }
        fs::remove_dir(path)?;
    } else {
        wipe_loop(path, params)?;
    }
    Ok(())
}

fn walk1(path: &Path, depth: i32, params: &mut Params) {
    if let Err(err) = walk(path, depth, params) {
        params.error_counter += 1;
        eprintln!("{}", err);
    }
}

struct Params {
    opts: Opts,
    values: Vec<String>,
    rng: ThreadRng,
    error_counter: i32,
    zero_block: Option<Vec<u8>>,
}

fn main() -> Result<()> {
    let params = &mut Params {
        opts: get_opts()?,
        values: make_values(),
        rng: rand::thread_rng(),
        error_counter: 0,
        zero_block: None,
    };
    assert_ne!(params.values.len(), 0);
    if params.opts.zero {
        params.zero_block = Some(vec![0; params.opts.block_size as usize]);
    }
    for file in &params.opts.files.clone() {
        walk1(file, 0, params);
    }
    if params.error_counter != 0 {
        return Err(format!("{} errors were during wiping", params.error_counter).into());
    }
    Ok(())
}
