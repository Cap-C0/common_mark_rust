use common_mark_rust::markdown_to_html;
use ntest::timeout;
use pretty_assertions::assert_eq;

include!(concat!(env!("OUT_DIR"), "/spec_tests.rs"));

#[test]
#[timeout(50)]
fn my_test_frac_sym() {
    assert_eq!(markdown_to_html("&frac34; "), "<p>¾</p>\n");
}
#[test]
#[timeout(50)]
fn my_test_ouml_sym() {
    assert_eq!(markdown_to_html("&ouml; "), "<p>ö</p>\n");
}

#[test]
#[timeout(50)]
fn my_test_simple() {
    assert_eq!(markdown_to_html("a"), "<p>a</p>\n");
}

#[test]
#[timeout(50)]
fn my_test_tick() {
    assert_eq!(markdown_to_html("\\`"), "<p>`</p>\n");
}

#[test]
#[timeout(50)]
fn my_test_chars() {
    assert_eq!(
        markdown_to_html("&nbsp &x; &#; &#x;"),
        "<p>&amp;nbsp &amp;x; &amp;#; &amp;#x;</p>\n"
    )
}
