// Temporary reproduction harness — DELETE before commit.
// Times Engine::fix at a single size point to estimate the bench's per-iter cost.

use marque_config::Config;
use marque_engine::{Engine, FixMode};
use std::time::Instant;

fn build_fix_input(target_bytes: usize) -> Vec<u8> {
    let violation = "SECRET//NF\n\n";
    let prose_block = concat!(
        "TOP SECRET//SCI//NOFORN\n\n",
        "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Sed do\n",
        "eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim\n",
        "ad minim veniam, quis nostrud exercitation ullamco laboris nisi.\n\n",
        "(S//NF) Portion mark with abbreviated dissem - valid portion form.\n\n",
        "CONFIDENTIAL//REL TO USA, GBR\n\n",
        "Duis aute irure dolor in reprehenderit in voluptate velit esse cillum\n",
        "dolore eu fugiat nulla pariatur. Excepteur sint occaecat cupidatat.\n\n",
    );
    let section_target = 10_900usize;
    let violation_bytes = violation.as_bytes();
    let prose_bytes = prose_block.as_bytes();
    let mut section = Vec::with_capacity(section_target + prose_bytes.len());
    section.extend_from_slice(violation_bytes);
    while section.len() < section_target {
        section.extend_from_slice(prose_bytes);
    }
    let prose_reps = (section_target.saturating_sub(violation_bytes.len())) / prose_bytes.len();
    section.truncate(violation_bytes.len() + prose_reps.max(1) * prose_bytes.len());

    let mut input = Vec::with_capacity(target_bytes + section.len());
    while input.len() < target_bytes {
        input.extend_from_slice(&section);
    }
    let complete_sections = target_bytes / section.len();
    input.truncate(complete_sections.max(1) * section.len());
    input
}

#[test]
#[ignore] // run explicitly with `cargo test -p marque-engine --test _throughput_repro -- --ignored --nocapture`
fn time_one_iter_each_size() {
    let engine = Engine::new(
        Config::default(),
        marque_engine::default_ruleset(),
        marque_engine::default_scheme(),
    )
    .unwrap();

    for size in [1_000_000, 5_000_000, 10_000_000, 50_000_000] {
        let input = build_fix_input(size);
        eprintln!("=== size={} bytes (actual={}) ===", size, input.len());
        let t0 = Instant::now();
        let res = engine.fix(&input, FixMode::Apply);
        let elapsed = t0.elapsed();
        eprintln!(
            "fix elapsed: {:?}  output_len={}  applied={}",
            elapsed,
            res.source.len(),
            res.applied.len()
        );
    }
}
