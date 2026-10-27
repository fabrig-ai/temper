use super::{LIMITS, tests};
use crate::{Failures, Format, Outcome, Unreadable, failed, report};

#[test]
fn junit_names_failed_and_errored_cases_in_order() {
    let file = br#"<?xml version="1.0"?>
        <testsuites><testsuite name="nextest">
        <testcase classname="suite" name="passes"/>
        <testcase classname="suite" name="z"><failure message="a > b">bad</failure></testcase>
        <testcase classname="suite" name="a"><error/></testcase>
        <testcase classname="suite" name="skip"><skipped/></testcase>
        <testcase classname="suite" name="a"><failure/></testcase>
        </testsuite></testsuites>"#;
    assert_eq!(report(Format::Junit, file, &LIMITS), Ok(tests(&["suite.a", "suite.z"])));
}

#[test]
fn junit_decodes_named_and_numeric_entities_in_attributes() {
    let file = br#"<testsuite>
        <testcase classname="&lt;&gt;" name="&amp;&quot;&apos;&#65;&#x3B2;"><failure/></testcase>
        <testcase name="alone"><error/></testcase>
        <testcase classname="" name="empty"><failure/></testcase>
        </testsuite>"#;
    assert_eq!(report(Format::Junit, file, &LIMITS), Ok(tests(&["<>.&\"'Aβ", "alone", "empty"])));
}

#[test]
fn malformed_junit_is_unreadable_even_without_a_failure() {
    let malformed: &[&[u8]] = &[
        b"<testsuite><testcase/></testsuite>",
        b"<testsuite><testcase name='a'",
        b"<testsuite><testcase name='a'><failure/></testsuite>",
        b"<testcase name='a'><failure/></testcase>",
        b"<testsuiteish/>",
        b"<testsuite><!-- unterminated",
        b"<testsuite><testcase name='&unknown;'/></testsuite>",
        b"<testsuite><testcase name='&#xD800;'/></testsuite>",
        b"<testsuite><testcase name='&#0;'/></testsuite>",
        b"<testsuite><testcase name=a/></testsuite>",
        b"<testsuite><testcase classname='x'/></testsuite>",
    ];
    for file in malformed {
        assert_eq!(report(Format::Junit, file, &LIMITS), Err(Unreadable));
    }
}

#[test]
fn comments_and_cdata_do_not_invent_failed_tests() {
    let file = br#"<testsuite>
        <!-- <testcase name="ghost"><failure/></testcase> -->
        <testcase name="passes"><![CDATA[<failure/>]]></testcase>
        </testsuite>"#;
    assert_eq!(report(Format::Junit, file, &LIMITS), Ok(tests(&[])));
}

#[test]
fn lines_drop_cr_and_blanks_sorting_unique_ids() {
    assert_eq!(report(Format::Lines, b"b\r\n\r\na\n\nb\na\r", &LIMITS), Ok(tests(&["a", "b"])));
}

#[test]
fn report_count_accepts_four_and_refuses_five_unique_ids() {
    let cases: &[(Format, &[u8], Failures)] = &[
        (Format::Lines, b"a\nb\nc\nd\na", tests(&["a", "b", "c", "d"])),
        (Format::Lines, b"a\nb\nc\nd\ne", Failures::Many),
        (Format::Junit, br#"<testsuite><testcase name="a"><failure/></testcase><testcase name="b"><failure/></testcase><testcase name="c"><failure/></testcase><testcase name="d"><failure/></testcase></testsuite>"#, tests(&["a", "b", "c", "d"])),
        (Format::Junit, br#"<testsuite><testcase name="a"><failure/></testcase><testcase name="b"><failure/></testcase><testcase name="c"><failure/></testcase><testcase name="d"><failure/></testcase><testcase name="e"><failure/></testcase></testsuite>"#, Failures::Many),
    ];
    for (format, file, expected) in cases {
        assert_eq!(report(*format, file, &LIMITS), Ok(expected.clone()));
    }
}

#[test]
fn report_id_accepts_sixteen_bytes_and_refuses_seventeen() {
    // T1's test_id_bytes bound is 16, including classname and separator.
    const AT_BOUND: &str = "1234567890123456";
    let cases: &[(Format, &[u8], Failures)] = &[
        (Format::Lines, AT_BOUND.as_bytes(), tests(&[AT_BOUND])),
        (Format::Lines, b"12345678901234567", Failures::Many),
        (
            Format::Junit,
            br#"<testsuite><testcase classname="1234567" name="90123456"><error/></testcase></testsuite>"#,
            tests(&["1234567.90123456"]),
        ),
        (
            Format::Junit,
            br#"<testsuite><testcase classname="1234567" name="901234567"><error/></testcase></testsuite>"#,
            Failures::Many,
        ),
        (
            Format::Junit,
            br#"<testsuite><testcase name="123456789012345&#54;"><error/></testcase></testsuite>"#,
            tests(&[AT_BOUND]),
        ),
    ];
    for (format, file, expected) in cases {
        assert_eq!(report(*format, file, &LIMITS), Ok(expected.clone()));
    }
}

#[test]
fn malformed_junit_stays_unreadable_after_an_oversized_id() {
    let file = br#"<testsuite><testcase name="12345678901234567"><failure/></testcase><testcase"#;
    assert_eq!(report(Format::Junit, file, &LIMITS), Err(Unreadable));
}

#[test]
fn failed_run_without_named_tests_is_a_fault() {
    for report in [None, Some(Err(Unreadable)), Some(Ok(tests(&[])))] {
        assert_eq!(failed(report), Outcome::Fault);
    }
}

#[test]
fn failed_run_with_named_or_many_tests_is_red() {
    for failures in [tests(&["a"]), Failures::Many] {
        assert_eq!(failed(Some(Ok(failures.clone()))), Outcome::Failed(failures));
    }
}
