#[test]
fn parity_predicates_agree_for_boundaries_and_a_dense_range() {
    for n in (0usize..65_536).chain([usize::MAX - 1, usize::MAX]) {
        assert_eq!(n % 2 != 0, !n.is_multiple_of(2), "{n}");
    }
}
