#[macro_use]
mod common;

use common::test_helper::hoard_test;
use hoard::command::Command;

hoard_test! {
    name: test_hoard_list,
    tester: tester,
    config_toml: common::BASE_CONFIG,
    {
        let expected = "anon_dir\nanon_file\nnamed\n";
        tester.run_command(Command::List, false).expect("list command should not fail");
        tester.assert_has_output(expected);
    }
}