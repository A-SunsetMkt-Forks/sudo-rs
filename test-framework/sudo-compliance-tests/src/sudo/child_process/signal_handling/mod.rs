use pretty_assertions::assert_eq;
use sudo_test::{Command, Env};

use crate::{
    SUDOERS_ALL_ALL_NOPASSWD, SUDOERS_NOT_USE_PTY, SUDOERS_ROOT_ALL_NOPASSWD,
    SUDOERS_USER_ALL_NOPASSWD, SUDOERS_USE_PTY, USERNAME,
};

macro_rules! dup {
    ($($(#[$attrs:meta])* $name:ident,)*) => {
        mod tty {
            $(
                #[test]
                $(#[$attrs])*
                fn $name() {
                    super::$name(true);
                }
            )*
        }

        mod no_tty {
            $(
                #[test]
                $(#[$attrs])*
                fn $name() {
                    super::$name(false);
                }
            )*
        }
    };
}

dup! {
    signal_sent_by_child_process_is_ignored,
    signal_is_forwarded_to_child,
    child_terminated_by_signal,
    sigtstp_works,
    sigalrm_terminates_command,
    sigchld_is_ignored,
}

// man sudo > Signal handling
// "As a special case, sudo will not relay signals that were sent by the command it is running."
fn signal_sent_by_child_process_is_ignored(tty: bool) {
    let script = include_str!("kill-sudo-parent.sh");

    let kill_sudo_parent = "/root/kill-sudo-parent.sh";
    let env = Env([SUDOERS_USER_ALL_NOPASSWD, SUDOERS_USE_PTY])
        .user(USERNAME)
        .file(kill_sudo_parent, script)
        .build();

    Command::new("sudo")
        .args(["sh", kill_sudo_parent])
        .as_user(USERNAME)
        .tty(tty)
        .output(&env)
        .assert_success();
}

fn signal_is_forwarded_to_child(tty: bool) {
    let expected = "got signal";
    let expects_signal = "/root/expects-signal.sh";
    let kill_sudo = "/root/kill-sudo.sh";
    let env = Env([SUDOERS_USER_ALL_NOPASSWD, SUDOERS_USE_PTY])
        .user(USERNAME)
        .file(expects_signal, include_str!("expects-signal.sh"))
        .file(kill_sudo, include_str!("kill-sudo.sh"))
        .build();

    let child = Command::new("sudo")
        .args(["sh", expects_signal, "TERM"])
        .as_user(USERNAME)
        .spawn(&env);

    Command::new("sh")
        .args([kill_sudo, "-TERM"])
        .tty(tty)
        .output(&env)
        .assert_success();

    let actual = child.wait().stdout();

    assert_eq!(expected, actual);
}

// man sudo > Exit value
// "If the command terminated due to receipt of a signal, sudo will send itself the same signal that terminated the command."
fn child_terminated_by_signal(tty: bool) {
    let env = Env([SUDOERS_USER_ALL_NOPASSWD, SUDOERS_USE_PTY])
        .user(USERNAME)
        .build();

    // child process sends SIGTERM to itself
    let output = Command::new("sudo")
        .args(["sh", "-c", "kill $$"])
        .as_user(USERNAME)
        .tty(tty)
        .output(&env);

    output.assert_exit_code(143);
    assert!(output.stderr().is_empty());
}

fn sigtstp_works(tty: bool) {
    const STOP_DELAY: u64 = 5;
    const NUM_ITERATIONS: usize = 5;

    let script_path = "/tmp/script.sh";
    let env = Env([SUDOERS_ALL_ALL_NOPASSWD, SUDOERS_USE_PTY])
        .file(script_path, include_str!("sigtstp.bash"))
        .build();

    let output = Command::new("bash")
        .arg(script_path)
        .tty(tty)
        .output(&env)
        .stdout();

    let timestamps = output
        .lines()
        .filter_map(|line| {
            // when testing the use_pty-enabled ogsudo we have observed a `\r\r\n` line ending,
            // instead of the regular `\r\n` line ending that the `lines` adapter will remove. use
            // `trim_end` to remove the `\r` that `lines` won't remove
            line.trim_end().parse::<u64>().ok()
        })
        .collect::<Vec<_>>();

    dbg!(&timestamps);

    assert_eq!(NUM_ITERATIONS, timestamps.len());

    let suspended_iterations = timestamps
        .windows(2)
        .filter(|window| {
            let prev_timestamp = window[0];
            let curr_timestamp = window[1];
            let delta = curr_timestamp - prev_timestamp;

            delta >= STOP_DELAY
        })
        .count();
    let did_suspend = suspended_iterations == 1;

    assert!(did_suspend);
}

fn sigalrm_terminates_command(tty: bool) {
    let expected = "got signal";
    let expects_signal = "/root/expects-signal.sh";
    let kill_sudo = "/root/kill-sudo.sh";
    let env = Env([SUDOERS_USER_ALL_NOPASSWD, SUDOERS_USE_PTY])
        .user(USERNAME)
        .file(expects_signal, include_str!("expects-signal.sh"))
        .file(kill_sudo, include_str!("kill-sudo.sh"))
        .build();

    let child = Command::new("sudo")
        .args(["sh", expects_signal, "HUP", "TERM"])
        .as_user(USERNAME)
        .spawn(&env);

    // Wait for expects-signal.sh to install the signal handler
    std::thread::sleep(std::time::Duration::from_secs(1));

    Command::new("sh")
        .args([kill_sudo, "-ALRM"])
        .tty(tty)
        .output(&env)
        .assert_success();

    let actual = child.wait().stdout();

    assert_eq!(expected, actual);
}

fn sigchld_is_ignored(tty: bool) {
    let expected = "got signal";
    let expects_signal = "/root/expects-signal.sh";
    let kill_sudo = "/root/kill-sudo.sh";
    let env = Env([SUDOERS_USER_ALL_NOPASSWD, SUDOERS_USE_PTY])
        .user(USERNAME)
        .file(expects_signal, include_str!("expects-signal.sh"))
        .file(kill_sudo, include_str!("kill-sudo.sh"))
        .build();

    let child = Command::new("sudo")
        .args(["sh", expects_signal, "HUP", "TERM"])
        .as_user(USERNAME)
        .spawn(&env);

    Command::new("sh")
        .args([kill_sudo, "-CHLD"])
        .tty(tty)
        .output(&env)
        .assert_success();

    Command::new("sh")
        .args([kill_sudo, "-ALRM"])
        .tty(tty)
        .output(&env)
        .assert_success();

    let actual = child.wait().stdout();

    assert_eq!(expected, actual);
}

fn sigwinch_works(use_pty: bool) {
    let print_sizes = "/root/print-sizes.sh";
    let change_size = "/root/change-size.sh";
    let env = Env([
        SUDOERS_ROOT_ALL_NOPASSWD,
        if use_pty {
            SUDOERS_USE_PTY
        } else {
            SUDOERS_NOT_USE_PTY
        },
    ])
    .file(print_sizes, include_str!("print-sizes.sh"))
    .file(change_size, include_str!("change-size.sh"))
    .build();

    let child = Command::new("sh").arg(print_sizes).tty(true).spawn(&env);

    Command::new("sh")
        .arg(change_size)
        .output(&env)
        .assert_success();

    let output = child.wait().stdout();

    let lines: Vec<_> = output.lines().collect();
    assert_eq!(lines.len(), 3);
    // Assert that the terminal size that sudo sees the first time matches the original terminal size.
    assert_eq!(lines[0], lines[1]);
    // Assert that the terminal size that sudo sees the second time has actually changed to the
    // value set by `change-size.sh`.
    assert_eq!(lines[2].trim(), "42 69");
}

#[test]
#[cfg_attr(
    target_os = "freebsd",
    ignore = "gh924: podman on freebsd puts each docker exec command in a jail with separate /dev/pts"
)]
fn sigwinch_works_pty() {
    sigwinch_works(true)
}

#[test]
#[cfg_attr(
    target_os = "freebsd",
    ignore = "gh924: podman on freebsd puts each docker exec command in a jail with separate /dev/pts"
)]
fn sigwinch_works_no_pty() {
    sigwinch_works(false)
}
