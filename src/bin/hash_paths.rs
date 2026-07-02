use b3hash::{DirectoryHasher, verify};
use std::{env, error, time};

fn main() -> Result<(), Box<dyn error::Error>> {
    println!("starting program");
    match env::args().len() {
        1 => println!("need a path to hash"),
        2 => single_path(env::args().nth(1).unwrap()),
        3 => double_path(env::args().nth(1).unwrap(), env::args().nth(2).unwrap())?,
        _ => println!("too many arguments"),
    }
    println!("program completed");
    Ok(())
}

fn single_path(path: String) {
    let hasher = DirectoryHasher::builder()
        .directory_path(path.into())
        .build();
    let start = time::Instant::now();
    let res = hasher.hash();
    let delta = time::Instant::now().duration_since(start).as_secs_f64();
    match res {
        Ok(_) => {
            println!("hashing successful, completed in:");
            println!("\t{:.2} seconds", delta);
            println!("\t{:.2} ms", delta * 1e3);
            println!("\t{:.2} us", delta * 1e6);
        }
        Err(_) => {
            println!("hashing failed after {:.4} seconds", delta);
        }
    }
}

fn double_path(path_1: String, path_2: String) -> Result<(), Box<dyn error::Error>> {
    let m1 = DirectoryHasher::builder()
        .directory_path(path_1.into())
        .build()
        .hash()?;
    let m2 = DirectoryHasher::builder()
        .directory_path(path_2.into())
        .build()
        .hash()?;
    match verify(m1, m2) {
        None => println!("no difference found"),
        Some(diff) => {
            println!("difference found");
            println!("{:#?}", diff)
        }
    }
    Ok(())
}
