#![allow(unused)]

use rayon::prelude::*;
use std::collections::HashSet;

const RADIX: u32 = 10;

fn aoc_2() {
    const INTS: [&str; 18] = [
        "1", "2", "3", "4", "5", "6", "7", "8", "9", "one", "two", "three", "four", "five", "six",
        "seven", "eight", "nine",
    ];

    fn parse(index: usize, string: &str) -> u32 {
        let string = string.get(index..).unwrap();
        if let Some(c) = (string.bytes().next().unwrap() as char).to_digit(RADIX) {
            return c;
        }
        for (i, int) in INTS.iter().skip(INTS.len() / 2).enumerate() {
            if string.starts_with(int) {
                return INTS[i].parse().unwrap();
            }
        }
        unreachable!()
    }

    let result = std::fs::read_to_string("input1.txt")
        .expect("input file should exist")
        .par_lines()
        .map(|s| {
            let first_idx = INTS.iter().filter_map(|int| s.find(int)).min().unwrap();
            let last_idx = INTS.iter().filter_map(|int| s.rfind(int)).max().unwrap();
            let first = parse(first_idx, s);
            let last = parse(last_idx, s);
            ((first * RADIX) + last) as u64
        })
        .sum::<u64>();
    println!("{}", result);
}

fn aoc_1() {
    fn find(mut c: impl Iterator<Item = char>) -> u32 {
        c.find(char::is_ascii_digit)
            .expect("there should be a value to find")
            .to_digit(RADIX)
            .expect("radix of 10 should be fine")
    }

    let result = std::fs::read_to_string("input1.txt")
        .expect("input file should exist")
        .par_lines()
        .map(|s| {
            let first = find(s.chars());
            let last = find(s.chars().rev());
            ((first * RADIX) + last) as u64
        })
        .sum::<u64>();
    println!("{}", result);
}

fn main() -> std::io::Result<()> {
    const MEG: f64 = (1 << 20) as f64;
    /*aoc_1();
    aoc_2();
    return Ok(());*/

    let path = std::env::args()
        .nth(1)
        .expect("please enter directory path");

    /*let (res, t) = time(|| b3hash::create_hashfile(&path));
    let _ = res?;
    println!("Execution time: {:.2}", t);
    //return Ok(());

    let (res, t) = time(|| b3hash::validate_hashfile(&path));
    let res = res?;
    if res.is_none() {
        println!("all files validated");
        println!("time: {:.2}", t);
    } else {
        println!("validation failed:");
        println!("{:?}", res.unwrap());
    }
    println!();
    return Ok(());*/

    let (res, time) = time(|| b3hash::hash_directory(&path));
    let res = res?;
    println!("Execution time: {:.2} seconds", time);
    println!("Directory name: {}", res.dir_name);
    println!("Directory checksum: {}", res.hash.to_hex());
    println!("File count: {}", res.len());
    println!("Final size in bytes: {}", res.size);
    println!("Final size in megabytes: {:.2}", res.size as f64 / 1e6);
    println!("Final size in gigabytes: {:.2}", res.size as f64 / 1e9);
    println!(
        "Execution speed: {:.2} MiB/s",
        (res.size) as f64 / time / MEG
    );
    println!();
    Ok(())
}

#[inline(always)]
fn time<F, R>(func: F) -> (R, f64)
where
    F: FnOnce() -> R,
{
    let start = std::time::Instant::now();
    let res = func();
    let time_delta = std::time::Instant::now()
        .duration_since(start)
        .as_secs_f64();
    (res, time_delta)
}
