pub fn assert_contract(screen_id: &str, output: &str, required: &[&str]) {
    for text in required {
        assert!(
            output.contains(text),
            "{screen_id} contract is missing {text:?}; output: {output:?}"
        );
    }
}
