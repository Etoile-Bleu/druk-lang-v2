use druk_parser::parse;

const SMALL_STACK_BYTES: usize = 1024 * 1024;

fn parse_on_small_stack(source: String) -> Result<usize, String> {
    let handle = std::thread::Builder::new()
        .stack_size(SMALL_STACK_BYTES)
        .spawn(move || parse(&source).1.len())
        .map_err(|error| error.to_string())?;
    handle
        .join()
        .map_err(|_| "the parser overflowed its stack or panicked".to_string())
}

#[test]
fn deeply_nested_brackets_give_one_clear_diagnostic() -> Result<(), String> {
    let depth = 200_000;
    let source = format!("{}༡{}", "(".repeat(depth), ")".repeat(depth));
    assert_eq!(parse_on_small_stack(source)?, 1);
    Ok(())
}

#[test]
fn deeply_nested_negations_give_one_clear_diagnostic() -> Result<(), String> {
    let source = format!("{}༡;", "-".repeat(200_000));
    assert_eq!(parse_on_small_stack(source)?, 1);
    Ok(())
}

#[test]
fn deeply_nested_blocks_give_one_clear_diagnostic() -> Result<(), String> {
    let depth = 50_000;
    let source = format!("{}{}", "གལ་སྲིད་ (བདེན་པ་) {".repeat(depth), "}".repeat(depth));
    assert_eq!(parse_on_small_stack(source)?, 1);
    Ok(())
}

#[test]
fn reasonable_nesting_still_parses_cleanly() -> Result<(), String> {
    let depth = 40;
    let source = format!("བཀོད་ {}༡{};", "(".repeat(depth), ")".repeat(depth));
    assert_eq!(parse_on_small_stack(source)?, 0);
    Ok(())
}
