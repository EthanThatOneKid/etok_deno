/// Returns true if the given character is a space character.
pub fn is_space_byte(c: char) -> bool {
  c == ' ' || c == '\t' || c == '\n' || c == '\r'
}

/// A simple shell-like string splitter that splits on spaces
/// unless the space is quoted.
///
/// https://tip.golang.org/src/cmd/internal/quoted/quoted.go
pub fn quoted_split(s: &str) -> Vec<&str> {
  // Split fields allowing '' or "" around elements.
  // Quotes further inside the string do not count.
  let mut f: Vec<&str> = vec![];
  let mut s = s;
  while s.len() > 0 {
    while s.len() > 0 && is_space_byte(s.chars().next().unwrap()) {
      s = &s[1..];
    }
    if s.len() == 0 {
      break;
    }
    // Accepted quoted string. No unescaping inside.
    if s.chars().next().unwrap() == '"' || s.chars().next().unwrap() == '\'' {
      let quote = s.chars().next().unwrap();
      s = &s[1..];
      let i = s.find(quote).unwrap_or_else(|| {
        panic!("unterminated {} string", quote);
      });
      f.push(&s[..i]);
      s = &s[i + 1..];
      continue;
    }
    let i = s.chars().position(|c| is_space_byte(c)).unwrap_or(s.len());
    f.push(&s[..i]);
    s = &s[i..];
  }
  f
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_empty_string() {
    let got = quoted_split("");
    let want: Vec<&str> = vec![];
    assert_eq!(got, want);
  }

  #[test]
  fn test_string_with_space() {
    let got = quoted_split(" ");
    let want: Vec<&str> = vec![];
    assert_eq!(got, want);
  }

  #[test]
  fn test_string_with_one_word() {
    let got = quoted_split("a");
    let want: Vec<&str> = vec!["a"];
    assert_eq!(got, want);
  }

  #[test]
  fn test_string_with_leading_space() {
    let got = quoted_split(" a");
    let want: Vec<&str> = vec!["a"];
    assert_eq!(got, want);
  }

  #[test]
  fn test_string_with_trailing_space() {
    let got = quoted_split("a ");
    let want: Vec<&str> = vec!["a"];
    assert_eq!(got, want);
  }

  #[test]
  fn test_string_with_two_words() {
    let got = quoted_split("a b");
    let want: Vec<&str> = vec!["a", "b"];
    assert_eq!(got, want);
  }

  #[test]
  fn test_string_with_two_words_and_multi_space() {
    let got = quoted_split("a  b");
    let want: Vec<&str> = vec!["a", "b"];
    assert_eq!(got, want);
  }

  #[test]
  fn test_string_with_two_words_and_tab() {
    let got = quoted_split("a\tb");
    let want: Vec<&str> = vec!["a", "b"];
    assert_eq!(got, want);
  }

  #[test]
  fn test_string_with_two_words_and_newline() {
    let got = quoted_split("a\nb");
    let want: Vec<&str> = vec!["a", "b"];
    assert_eq!(got, want);
  }

  #[test]
  fn test_string_with_single_quoted_word() {
    let got = quoted_split("'a b'");
    let want: Vec<&str> = vec!["a b"];
    assert_eq!(got, want);
  }

  #[test]
  fn test_string_with_double_quoted_word() {
    let got = quoted_split(r#""a b""#);
    let want: Vec<&str> = vec!["a b"];
    assert_eq!(got, want);
  }

  #[test]
  fn test_string_with_both_quoted_words() {
    let got = quoted_split(r#"'a '"b ""#);
    let want: Vec<&str> = vec!["a ", "b "];
    assert_eq!(got, want);
  }

  #[test]
  fn test_string_with_quotes_contained_within_each_other() {
    let got = quoted_split(r#"'a "'"'b""#);
    let want: Vec<&str> = vec![r#"a ""#, r#"b"#];
    assert_eq!(got, want);
  }

  #[test]
  fn test_escaped_single_quote() {
    let got = quoted_split(r#"\'"#);
    let want: Vec<&str> = vec![r#"\'"#];
    assert_eq!(got, want);
  }

  #[test]
  fn test_unterminated_single_quote() {
    assert!(quoted_split("'a").is_err());
  }
}
