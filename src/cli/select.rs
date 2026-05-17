use crate::{auth::AuthInfo, storage::AccountRecord};

use super::CliError;

pub(super) fn resolve_selector(
    accounts: &[AccountRecord],
    selector: &str,
) -> Result<usize, CliError> {
    if let Ok(number) = selector.parse::<usize>() {
        if number == 0 || number > accounts.len() {
            return Err(CliError::Message(format!("no account at row {number}")));
        }
        return Ok(number - 1);
    }

    let matches: Vec<usize> = accounts
        .iter()
        .enumerate()
        .filter_map(|(index, account)| {
            account
                .email
                .eq_ignore_ascii_case(selector)
                .then_some(index)
        })
        .collect();
    match matches.as_slice() {
        [] => Err(CliError::Message(format!(
            "no account matches exact email `{selector}`"
        ))),
        [index] => Ok(*index),
        _ => Err(CliError::Message(format!(
            "email `{selector}` matches multiple accounts; use a row number"
        ))),
    }
}

pub(super) fn display_label(info: &AuthInfo, duplicate: bool) -> String {
    if duplicate {
        format!("{} ({})", info.email, short_account_id(&info.account_id))
    } else {
        info.email.clone()
    }
}

pub(super) fn display_record_label(account: &AccountRecord, duplicate: bool) -> String {
    if duplicate {
        format!(
            "{} ({})",
            account.email,
            short_account_id(&account.chatgpt_account_id)
        )
    } else {
        account.email.clone()
    }
}

fn short_account_id(account_id: &str) -> String {
    account_id.chars().take(8).collect()
}

pub(super) fn duplicate_record_email(accounts: &[AccountRecord], index: usize) -> bool {
    accounts
        .iter()
        .filter(|account| account.email == accounts[index].email)
        .count()
        > 1
}

#[cfg(test)]
mod tests {
    use crate::{cli::test_support::save_account, storage};

    use super::*;

    #[test]
    fn resolves_rows_and_exact_email_selectors() {
        let temp = tempfile::TempDir::new().unwrap();
        save_account(&temp, "a@example.com", "user-a", "acct-a");
        save_account(&temp, "b@example.com", "user-b", "acct-b");
        let registry = storage::load_registry(temp.path()).unwrap();

        assert_eq!(resolve_selector(&registry.accounts, "1").unwrap(), 0);
        assert_eq!(
            resolve_selector(&registry.accounts, "b@example.com").unwrap(),
            1
        );
    }

    #[test]
    fn rejects_ambiguous_email_selectors() {
        let temp = tempfile::TempDir::new().unwrap();
        save_account(&temp, "same@example.com", "user-a", "acct-a");
        save_account(&temp, "same@example.com", "user-b", "acct-b");
        let registry = storage::load_registry(temp.path()).unwrap();

        let error = resolve_selector(&registry.accounts, "same@example.com").unwrap_err();
        assert!(error.to_string().contains("matches multiple accounts"));
    }

    #[test]
    fn duplicate_emails_get_disambiguated_labels() {
        let temp = tempfile::TempDir::new().unwrap();
        save_account(&temp, "same@example.com", "user-a", "acct-alpha");
        save_account(&temp, "same@example.com", "user-b", "acct-beta");
        let registry = storage::load_registry(temp.path()).unwrap();

        assert_eq!(
            display_record_label(
                &registry.accounts[0],
                duplicate_record_email(&registry.accounts, 0)
            ),
            "same@example.com (acct-alp)"
        );
        assert_eq!(
            display_record_label(
                &registry.accounts[1],
                duplicate_record_email(&registry.accounts, 1)
            ),
            "same@example.com (acct-bet)"
        );
    }
}
