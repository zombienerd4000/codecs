use std::env;
use std::fs;
use std::time::Instant;
use tkc1::{compress, decompress};
use flate2::read::GzEncoder;
use flate2::Compression;

fn bench_codec<F: Fn(&[u8]) -> Vec<u8>, G: Fn(&[u8]) -> Vec<u8>>(
    label: &str, data: &[u8], enc: F, dec: G,
) {
    let t0 = Instant::now();
    let c = enc(data);
    let et = t0.elapsed();
    let t1 = Instant::now();
    let dec_data = dec(&c);
    let dt = t1.elapsed();
    let ok = if dec_data == data { "OK" } else { "FAIL" };
    let pct = 100.0 * c.len() as f64 / data.len() as f64;
    println!("{:<20} {:>6}B -> {:>6}B ({:>5.1}%) enc:{:>8.1}ms dec:{:>6.1}ms [{}]",
             label, data.len(), c.len(), pct,
             et.as_secs_f64() * 1000.0, dt.as_secs_f64() * 1000.0, ok);
}

fn gzip_enc(data: &[u8]) -> Vec<u8> {
    use std::io::Read;
    let mut enc = GzEncoder::new(data, Compression::best());
    let mut out = Vec::new();
    enc.read_to_end(&mut out).unwrap();
    out
}

fn gzip_dec(data: &[u8]) -> Vec<u8> {
    use flate2::read::GzDecoder;
    use std::io::Read;
    let mut dec = GzDecoder::new(data);
    let mut out = Vec::new();
    dec.read_to_end(&mut out).unwrap();
    out
}

fn zstd_enc(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    zstd::stream::copy_encode(data, &mut out, 19).unwrap();
    out
}

fn zstd_dec(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    zstd::stream::copy_decode(data, &mut out).unwrap();
    out
}

#[allow(dead_code)]
fn tlz3_enc(data: &[u8]) -> Vec<u8> {
    use std::io::Write;
    let script = r"C:\Users\komori\Documents\super cluster\pawnet\tlz3.py";
    let mut child = std::process::Command::new("python")
        .arg(script).arg("c")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .expect("could not launch tlz3.py");
    child.stdin.take().unwrap().write_all(data).unwrap();
    child.wait_with_output().unwrap().stdout
}

fn tlz3_dec(data: &[u8]) -> Vec<u8> {
    use std::io::Write;
    let script = r"C:\Users\komori\Documents\super cluster\pawnet\tlz3.py";
    let mut child = std::process::Command::new("python")
        .arg(script).arg("d")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .expect("could not launch tlz3.py");
    child.stdin.take().unwrap().write_all(data).unwrap();
    child.wait_with_output().unwrap().stdout
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("tkc1: Transform-invariant LZ77 + adaptive rANS codec");
        eprintln!("Usage:");
        eprintln!("  tkc1 c <input> [output]   Compress");
        eprintln!("  tkc1 d <input> [output]   Decompress");
        eprintln!("  tkc1 t <input>            Test round-trip");
        eprintln!("  tkc1 bench                Run benchmarks");
        return;
    }

    match args[1].as_str() {
        "c" => {
            let data = fs::read(&args[2]).expect("read input");
            let t0 = Instant::now();
            let out = compress(&data);
            let elapsed = t0.elapsed();
            let dest = if args.len() > 3 { args[3].clone() }
                       else { format!("{}.tkc1", args[2]) };
            fs::write(&dest, &out).expect("write output");
            let pct = 100.0 * out.len() as f64 / data.len() as f64;
            eprintln!("{} -> {} ({:.1}%, {:.1}ms)", args[2], dest, pct,
                      elapsed.as_secs_f64() * 1000.0);
        }
        "d" => {
            let data = fs::read(&args[2]).expect("read input");
            let t0 = Instant::now();
            let out = decompress(&data);
            let elapsed = t0.elapsed();
            let dest = if args.len() > 3 { args[3].clone() }
                       else { args[2].trim_end_matches(".tkc1").to_string() };
            fs::write(&dest, &out).expect("write output");
            eprintln!("{} -> {} ({}B, {:.1}ms)", args[2], dest, out.len(),
                      elapsed.as_secs_f64() * 1000.0);
        }
        "t" => {
            let data = fs::read(&args[2]).expect("read input");
            let t0 = Instant::now();
            let c = compress(&data);
            let ct = t0.elapsed();
            let t1 = Instant::now();
            let dec = decompress(&c);
            let dt = t1.elapsed();
            let ok = if dec == data { "OK" } else { "FAIL" };
            let pct = 100.0 * c.len() as f64 / data.len() as f64;
            eprintln!("{}: {}B -> {}B ({:.1}%) enc:{:.1}ms dec:{:.1}ms [{}]",
                      args[2], data.len(), c.len(), pct,
                      ct.as_secs_f64() * 1000.0,
                      dt.as_secs_f64() * 1000.0, ok);
        }
        _ => run_bench(),
    }
}

fn run_bench() {
    use std::f64::consts::PI;

    let text = b"Hello World! Hello World! hELLO wORLD!";
    let base: Vec<u8> = (0..=255).collect();
    let shifted: Vec<u8> = base.iter().map(|&b| b.wrapping_add(10)).collect();
    let subbed: Vec<u8> = base.iter().map(|&b| b.wrapping_sub(10)).collect();
    let repeated: Vec<u8> = b"abcdefghijklmnop".repeat(500);
    let pat: Vec<u8> = vec![0x10, 0x20, 0x30, 0x40, 0x30, 0x40, 0x50, 0x60, 0x90, 0xA0, 0xB0, 0xC0];
    let mixed: Vec<u8> = pat.repeat(100);

    fn lcg(seed: &mut u64) -> u8 {
        *seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (*seed >> 32) as u8
    }
    let mut seed: u64 = 42;
    let rand_data: Vec<u8> = (0..10000).map(|_| lcg(&mut seed)).collect();

    let sine: Vec<u8> = (0..4096)
        .map(|i| (128.0 + 100.0 * (2.0 * PI * i as f64 / 64.0).sin()).round() as u8)
        .collect();

    let all_base: Vec<u8> = base.iter().chain(&shifted).copied().collect();
    let all_sub: Vec<u8> = base.iter().chain(&subbed).copied().collect();

    // real files
    let real_files: [(&str, &str); 15] = [
        ("tlz3.py",          r"C:\Users\komori\Documents\super cluster\pawnet\tlz3.py"),
        ("rans.rs",          r"C:\Users\komori\Documents\super cluster\tkc1\src\rans.rs"),
        ("tiktok.py",        r"C:\Users\komori\AppData\Local\Alexa\Virtual\Lib\site-packages\youtube_dl\extractor\tiktok.py"),
        ("LICENSE.txt",      r"C:\Users\komori\AppData\Local\Alexa\Virtual\LICENSE.txt"),
        ("README.txt",       r"C:\Users\komori\AppData\Local\Alexa\Virtual\README.txt"),
        ("app.py",           r"C:\Users\komori\Desktop\komori_portfolio\app.py"),
        ("pawnet.html",      r"test_data\pawnet_monitor.html"),
        ("spine64_README",   r"test_data\README.md"),
        ("spine64_SPEC",     r"test_data\SPEC.md"),
        ("chat_sitemap.xml", r"test_data\chat_sitemap.xml"),
        ("chat_rules.txt",   r"test_data\chat_rules.txt"),
        ("chat_messages.txt",r"test_data\chat_messages.txt"),
        ("chat_cli_code.txt",r"test_data\chat_cli_code.txt"),
        ("chat_pages.txt",   r"test_data\chat_pages.txt"),
        ("ddlc_log.txt",     r"test_data\ddlc_log.txt"),
    ];

    let tests: [(&str, &[u8]); 7] = [
        ("XOR text", text),
        ("ADD shift", &all_base),
        ("SUB shift", &all_sub),
        ("mixed", &mixed),
        ("exact", &repeated),
        ("sine", &sine),
        ("random10k", &rand_data),
    ];

    // load real files
    let mut real_data: Vec<(&str, Vec<u8>)> = Vec::new();
    for (label, path) in &real_files {
        match std::fs::read(path) {
            Ok(d) => { real_data.push((label, d)); }
            Err(e) => { eprintln!("  skipping {}: {}", label, e); }
        }
    }

    let codecs: [(&str, fn(&[u8]) -> Vec<u8>, fn(&[u8]) -> Vec<u8>); 3] = [
        ("--- tkc1 ---", compress, decompress),
        ("--- gzip -9 ---", gzip_enc, gzip_dec),
        ("--- zstd -19 ---", zstd_enc, zstd_dec),
    ];

    for (header, enc, dec) in &codecs {
        println!("{}", header);
        for (label, data) in &tests {
            bench_codec(label, data, *enc, *dec);
        }
    }

    // real file benchmarks (only tkc1 and zstd for brevity)
    println!("\n--- real files (tkc1) ---");
    for (label, data) in &real_data {
        bench_codec(label, data, compress, decompress);
    }
    println!("--- real files (zstd -19) ---");
    for (label, data) in &real_data {
        bench_codec(label, data, zstd_enc, zstd_dec);
    }
    println!("--- real files (gzip -9) ---");
    for (label, data) in &real_data {
        bench_codec(label, data, gzip_enc, gzip_dec);
    }
}
