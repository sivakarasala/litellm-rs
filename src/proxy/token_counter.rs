use rust_decimal::Decimal;
use std::str::FromStr;

/// Model pricing per 1M tokens (input, output).
pub fn model_pricing(model: &str) -> (Decimal, Decimal) {
    // Prices per 1M tokens as of 2025
    match model {
        // GPT-4.1 family
        m if m.starts_with("gpt-4.1-nano") => (dec("0.10"), dec("0.40")),
        m if m.starts_with("gpt-4.1-mini") => (dec("0.40"), dec("1.60")),
        m if m.starts_with("gpt-4.1") => (dec("2.00"), dec("8.00")),
        // GPT-4o family
        m if m.starts_with("gpt-4o-mini") => (dec("0.15"), dec("0.60")),
        m if m.starts_with("gpt-4o") => (dec("2.50"), dec("10.00")),
        // GPT-4 Turbo
        m if m.starts_with("gpt-4-turbo") => (dec("10.00"), dec("30.00")),
        // GPT-4
        m if m.starts_with("gpt-4-32k") => (dec("60.00"), dec("120.00")),
        m if m.starts_with("gpt-4") => (dec("30.00"), dec("60.00")),
        // GPT-5 family
        m if m.starts_with("gpt-5-mini") => (dec("1.10"), dec("4.40")),
        m if m.starts_with("gpt-5") => (dec("10.00"), dec("40.00")),
        // GPT-3.5
        m if m.starts_with("gpt-3.5-turbo") => (dec("0.50"), dec("1.50")),
        // o4-mini
        m if m.starts_with("o4-mini") => (dec("1.10"), dec("4.40")),
        // o3 family
        m if m.starts_with("o3-mini") => (dec("1.10"), dec("4.40")),
        m if m.starts_with("o3") => (dec("10.00"), dec("40.00")),
        // o1 family
        m if m.starts_with("o1-mini") => (dec("3.00"), dec("12.00")),
        m if m.starts_with("o1") => (dec("15.00"), dec("60.00")),
        // Embeddings
        m if m.contains("embedding-3-large") => (dec("0.13"), dec("0.00")),
        m if m.contains("embedding-3-small") => (dec("0.02"), dec("0.00")),
        m if m.contains("embedding") => (dec("0.10"), dec("0.00")),
        // Anthropic Claude family (per 1M tokens, verified July 2026)
        // Legacy Opus (4.1 and earlier) predates the price cut
        m if m.starts_with("claude-opus-4-1") => (dec("15.00"), dec("75.00")),
        m if m.starts_with("claude-opus-4-2") => (dec("15.00"), dec("75.00")),
        m if m.starts_with("claude-3-opus") => (dec("15.00"), dec("75.00")),
        // Current Opus generation (4.5+)
        m if m.starts_with("claude-opus") => (dec("5.00"), dec("25.00")),
        // Sonnet has held $3/$15 across generations (3.5 through 4.6)
        m if m.starts_with("claude-sonnet") => (dec("3.00"), dec("15.00")),
        m if m.starts_with("claude-3-5-sonnet") => (dec("3.00"), dec("15.00")),
        m if m.starts_with("claude-3-7-sonnet") => (dec("3.00"), dec("15.00")),
        // Haiku
        m if m.starts_with("claude-haiku-4") => (dec("1.00"), dec("5.00")),
        m if m.starts_with("claude-3-5-haiku") => (dec("0.80"), dec("4.00")),
        m if m.starts_with("claude-3-haiku") => (dec("0.25"), dec("1.25")),
        m if m.starts_with("claude-haiku") => (dec("1.00"), dec("5.00")),
        // Conservative fallback for unrecognized Claude models: bill at
        // the current top published tier so budgets fail safe.
        m if m.starts_with("claude-") => (dec("10.00"), dec("50.00")),
        // Default fallback
        _ => (dec("1.00"), dec("3.00")),
    }
}

/// Prompt-cache pricing multipliers relative to base input price.
/// Cache writes (5-minute TTL) are billed at 1.25x input; cache reads at 0.1x.
const CACHE_WRITE_MULTIPLIER: &str = "1.25";
const CACHE_READ_MULTIPLIER: &str = "0.1";

/// Calculate cost in USD including prompt-cache token pricing.
///
/// `input_tokens` here means *uncached* input tokens (Anthropic reports
/// them separately from cache reads/writes in the `usage` object).
pub fn calculate_cost_with_cache(
    model: &str,
    input_tokens: u32,
    output_tokens: u32,
    cache_write_tokens: u32,
    cache_read_tokens: u32,
) -> Decimal {
    let (input_price, output_price) = model_pricing(model);
    let million = dec("1000000");
    let base = input_price * Decimal::from(input_tokens) / million
        + output_price * Decimal::from(output_tokens) / million;
    let cache_write =
        input_price * dec(CACHE_WRITE_MULTIPLIER) * Decimal::from(cache_write_tokens) / million;
    let cache_read =
        input_price * dec(CACHE_READ_MULTIPLIER) * Decimal::from(cache_read_tokens) / million;
    base + cache_write + cache_read
}

/// Calculate cost in USD from token counts.
pub fn calculate_cost(model: &str, input_tokens: u32, output_tokens: u32) -> Decimal {
    let (input_price, output_price) = model_pricing(model);
    let million = dec("1000000");
    let input_cost = input_price * Decimal::from(input_tokens) / million;
    let output_cost = output_price * Decimal::from(output_tokens) / million;
    input_cost + output_cost
}

fn dec(s: &str) -> Decimal {
    Decimal::from_str(s).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gpt4o_pricing() {
        let (input, output) = model_pricing("gpt-4o");
        assert_eq!(input, dec("2.50"));
        assert_eq!(output, dec("10.00"));
    }

    #[test]
    fn gpt4o_mini_pricing() {
        let (input, output) = model_pricing("gpt-4o-mini");
        assert_eq!(input, dec("0.15"));
        assert_eq!(output, dec("0.60"));
    }

    #[test]
    fn embedding_pricing() {
        let (input, output) = model_pricing("text-embedding-3-small");
        assert_eq!(input, dec("0.02"));
        assert_eq!(output, dec("0.00"));
    }

    #[test]
    fn unknown_model_has_fallback_pricing() {
        let (input, output) = model_pricing("some-unknown-model");
        assert_eq!(input, dec("1.00"));
        assert_eq!(output, dec("3.00"));
    }

    #[test]
    fn calculate_cost_gpt4o() {
        // 1000 input tokens + 500 output tokens at gpt-4o pricing
        // Input: 2.50 / 1M * 1000 = 0.0025
        // Output: 10.00 / 1M * 500 = 0.005
        // Total: 0.0075
        let cost = calculate_cost("gpt-4o", 1000, 500);
        assert_eq!(cost, dec("0.0075"));
    }

    #[test]
    fn calculate_cost_zero_tokens() {
        let cost = calculate_cost("gpt-4o", 0, 0);
        assert_eq!(cost, dec("0"));
    }

    #[test]
    fn calculate_cost_embedding() {
        // Embeddings have no output cost
        let cost = calculate_cost("text-embedding-3-small", 1000, 0);
        assert_eq!(cost, dec("0.00002"));
    }

    #[test]
    fn gpt4o_variant_matches() {
        // gpt-4o-2024-08-06 should match gpt-4o pricing
        let (input, _) = model_pricing("gpt-4o-2024-08-06");
        assert_eq!(input, dec("2.50"));
    }

    #[test]
    fn claude_sonnet_pricing() {
        let (input, output) = model_pricing("claude-sonnet-4-6");
        assert_eq!(input, dec("3.00"));
        assert_eq!(output, dec("15.00"));
    }

    #[test]
    fn claude_current_opus_pricing() {
        let (input, output) = model_pricing("claude-opus-4-8");
        assert_eq!(input, dec("5.00"));
        assert_eq!(output, dec("25.00"));
    }

    #[test]
    fn claude_legacy_opus_pricing() {
        let (input, output) = model_pricing("claude-opus-4-1");
        assert_eq!(input, dec("15.00"));
        assert_eq!(output, dec("75.00"));
    }

    #[test]
    fn claude_haiku_pricing() {
        let (input, output) = model_pricing("claude-haiku-4-5-20251001");
        assert_eq!(input, dec("1.00"));
        assert_eq!(output, dec("5.00"));
    }

    #[test]
    fn claude_unknown_falls_back_to_top_tier() {
        let (input, output) = model_pricing("claude-future-model-9");
        assert_eq!(input, dec("10.00"));
        assert_eq!(output, dec("50.00"));
    }

    #[test]
    fn cache_cost_calculation() {
        // Sonnet: 1000 uncached input, 500 output, 2000 cache write, 8000 cache read
        // input:       3.00/1M * 1000          = 0.003
        // output:     15.00/1M * 500           = 0.0075
        // cache write: 3.00 * 1.25/1M * 2000   = 0.0075
        // cache read:  3.00 * 0.1/1M  * 8000   = 0.0024
        // total = 0.0204
        let cost = calculate_cost_with_cache("claude-sonnet-4-6", 1000, 500, 2000, 8000);
        assert_eq!(cost, dec("0.0204"));
    }

    #[test]
    fn cache_cost_without_cache_matches_base() {
        let with_cache = calculate_cost_with_cache("claude-sonnet-4-6", 1000, 500, 0, 0);
        let base = calculate_cost("claude-sonnet-4-6", 1000, 500);
        assert_eq!(with_cache, base);
    }
}
