use std::env;

const HELP_TEXT: &str = "server-ops updates available system packages on configured servers.\n\
\n\
Usage:\n\
  server-ops [--help]\n\
\n\
No server actions are performed yet.";

fn main() {
    let arguments = env::args().skip(1).collect::<Vec<_>>();

    if arguments.iter().any(|argument| argument == "--help") {
        println!("{HELP_TEXT}");
    }
}

#[cfg(test)]
mod tests {
    use super::HELP_TEXT;

    #[test]
    fn help_text_describes_the_utility() {
        assert!(HELP_TEXT.contains("updates available system packages"));
        assert!(HELP_TEXT.contains("server-ops [--help]"));
    }
}
