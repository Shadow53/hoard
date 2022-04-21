#[macro_use]
mod common;

use common::tester::Tester;
use hoard::command::Command;

#[tokio::test]
async fn test_hoard_list() {
    let tester = Tester::new(common::base::BASE_CONFIG).await;
    let expected = "anon_dir\nanon_file\nnamed\n";
    tester.expect_command(Command::List).await;
    tester.assert_has_output(expected);
}
