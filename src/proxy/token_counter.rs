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
        // Default fallback
        _ => (dec("1.00"), dec("3.00")),
    }
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
}
