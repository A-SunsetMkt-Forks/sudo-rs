use crate::common::resolve::AuthUser;
use crate::common::{HARDENED_ENUM_VALUE_0, HARDENED_ENUM_VALUE_1, HARDENED_ENUM_VALUE_2};
use crate::sudoers::AuthenticatingUser;
use crate::system::{Group, Hostname, Process, User};

use super::resolve::CurrentUser;
use super::{
    command::CommandAndArguments,
    resolve::{resolve_launch_and_shell, resolve_target_user_and_group},
    Error, SudoPath, SudoString,
};

#[derive(Clone, Copy)]
pub enum ContextAction {
    List,
    Run,
    Validate,
}

// this is a bit of a hack to keep the existing `Context` API working
pub struct OptionsForContext {
    pub chdir: Option<SudoPath>,
    pub group: Option<SudoString>,
    pub login: bool,
    pub non_interactive: bool,
    pub positional_args: Vec<String>,
    pub reset_timestamp: bool,
    pub shell: bool,
    pub stdin: bool,
    pub user: Option<SudoString>,
    pub action: ContextAction,
}

#[derive(Debug)]
pub struct Context {
    // cli options
    pub launch: LaunchType,
    pub chdir: Option<SudoPath>,
    pub command: CommandAndArguments,
    pub target_user: User,
    pub target_group: Group,
    pub stdin: bool,
    pub non_interactive: bool,
    pub use_session_records: bool,
    // system
    pub hostname: Hostname,
    pub current_user: CurrentUser,
    pub auth_user: AuthUser,
    pub process: Process,
    // policy
    pub use_pty: bool,
    pub password_feedback: bool,
}

#[derive(Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum LaunchType {
    Direct = HARDENED_ENUM_VALUE_0,
    Shell = HARDENED_ENUM_VALUE_1,
    Login = HARDENED_ENUM_VALUE_2,
}

impl Context {
    pub fn build_from_options(
        sudo_options: OptionsForContext,
        path: String,
        auth_user: AuthenticatingUser,
    ) -> Result<Context, Error> {
        let hostname = Hostname::resolve();
        let current_user = CurrentUser::resolve()?;
        let auth_user = match auth_user {
            AuthenticatingUser::InvokingUser => AuthUser::from_current_user(current_user.clone()),
            AuthenticatingUser::Root => AuthUser::resolve_root_for_rootpw()?,
        };
        let (target_user, target_group) =
            resolve_target_user_and_group(&sudo_options.user, &sudo_options.group, &current_user)?;
        let (launch, shell) = resolve_launch_and_shell(&sudo_options, &current_user, &target_user);
        let command = match sudo_options.action {
            ContextAction::Validate | ContextAction::List
                if sudo_options.positional_args.is_empty() =>
            {
                // FIXME `Default` is being used as `Option::None`
                Default::default()
            }
            _ => CommandAndArguments::build_from_args(shell, sudo_options.positional_args, &path),
        };

        Ok(Context {
            hostname,
            command,
            current_user,
            auth_user,
            target_user,
            target_group,
            use_session_records: !sudo_options.reset_timestamp,
            launch,
            chdir: sudo_options.chdir,
            stdin: sudo_options.stdin,
            non_interactive: sudo_options.non_interactive,
            process: Process::new(),
            use_pty: true,
            password_feedback: false,
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        sudo::SudoAction,
        sudoers::AuthenticatingUser,
        system::{interface::UserId, Hostname},
    };
    use std::collections::HashMap;

    use super::Context;

    #[test]
    fn test_build_context() {
        let options = SudoAction::try_parse_from(["sudo", "echo", "hello"])
            .unwrap()
            .try_into_run()
            .ok()
            .unwrap();
        let path = "/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin";
        let (ctx_opts, _pipe_opts) = options.into();
        let context = Context::build_from_options(
            ctx_opts,
            path.to_string(),
            AuthenticatingUser::InvokingUser,
        )
        .unwrap();

        let mut target_environment = HashMap::new();
        target_environment.insert("SUDO_USER".to_string(), context.current_user.name.clone());

        if cfg!(target_os = "linux") {
            // this assumes /bin is a symlink on /usr/bin, like it is on modern Debian/Ubuntu
            assert_eq!(context.command.command.to_str().unwrap(), "/usr/bin/echo");
        } else {
            assert_eq!(context.command.command.to_str().unwrap(), "/bin/echo");
        }
        assert_eq!(context.command.arguments, ["hello"]);
        assert_eq!(context.hostname, Hostname::resolve());
        assert_eq!(context.target_user.uid, UserId::ROOT);
    }
}
