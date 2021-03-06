use nom::{
    branch::alt,
    bytes::streaming::{tag, tag_no_case, take, take_while, take_while1},
    character::streaming::{char, digit1},
    combinator::{map, map_res},
    multi::{separated_list, separated_nonempty_list},
    sequence::{delimited, tuple},
    IResult,
};

use std::str::{from_utf8, FromStr};

// ----- number -----

// number          = 1*DIGIT
//                    ; Unsigned 32-bit integer
//                    ; (0 <= n < 4,294,967,296)
pub fn number(i: &[u8]) -> IResult<&[u8], u32> {
    let (i, bytes) = digit1(i)?;
    match from_utf8(bytes).ok().and_then(|s| u32::from_str(s).ok()) {
        Some(v) => Ok((i, v)),
        None => Err(nom::Err::Error(nom::error::make_error(
            i,
            nom::error::ErrorKind::ParseTo,
        ))),
    }
}

// same as `number` but 64-bit
pub fn number_64(i: &[u8]) -> IResult<&[u8], u64> {
    let (i, bytes) = digit1(i)?;
    match from_utf8(bytes).ok().and_then(|s| u64::from_str(s).ok()) {
        Some(v) => Ok((i, v)),
        None => Err(nom::Err::Error(nom::error::make_error(
            i,
            nom::error::ErrorKind::ParseTo,
        ))),
    }
}

// ----- string -----

// string = quoted / literal
pub fn string(i: &[u8]) -> IResult<&[u8], &[u8]> {
    alt((quoted, literal))(i)
}

// string bytes as utf8
pub fn string_utf8(i: &[u8]) -> IResult<&[u8], &str> {
    map_res(string, from_utf8)(i)
}

// quoted = DQUOTE *QUOTED-CHAR DQUOTE
pub fn quoted(i: &[u8]) -> IResult<&[u8], &[u8]> {
    delimited(char('"'), quoted_data, char('"'))(i)
}

// quoted bytes as utf8
pub fn quoted_utf8(i: &[u8]) -> IResult<&[u8], &str> {
    map_res(quoted, from_utf8)(i)
}

// QUOTED-CHAR = <any TEXT-CHAR except quoted-specials> / "\" quoted-specials
pub fn quoted_data(i: &[u8]) -> IResult<&[u8], &[u8]> {
    // Ideally this should use nom's `escaped` macro, but it suffers from broken
    // type inference unless compiled with the verbose-errors feature enabled.
    let mut escape = false;
    let mut len = 0;
    for c in i {
        if *c == b'"' && !escape {
            break;
        }
        len += 1;
        if *c == b'\\' && !escape {
            escape = true
        } else if escape {
            escape = false;
        }
    }
    Ok((&i[len..], &i[..len]))
}

// quoted-specials = DQUOTE / "\"
pub fn is_quoted_specials(c: u8) -> bool {
    c == b'"' || c == b'\\'
}

/// literal = "{" number "}" CRLF *CHAR8
///            ; Number represents the number of CHAR8s
pub fn literal(input: &[u8]) -> IResult<&[u8], &[u8]> {
    let parser = tuple((tag(b"{"), number, tag(b"}"), tag("\r\n")));

    let (remaining, (_, count, _, _)) = parser(input)?;

    let (remaining, data) = take(count)(remaining)?;

    if !data.iter().all(|byte| is_char8(*byte)) {
        // FIXME: what ErrorKind should this have?
        return Err(nom::Err::Error((remaining, nom::error::ErrorKind::Verify)));
    }

    Ok((remaining, data))
}

/// CHAR8 = %x01-ff ; any OCTET except NUL, %x00
pub fn is_char8(i: u8) -> bool {
    i != 0
}

// ----- astring ----- atom (roughly) or string

// astring = 1*ASTRING-CHAR / string
pub fn astring(i: &[u8]) -> IResult<&[u8], &[u8]> {
    alt((take_while1(is_astring_char), string))(i)
}

// astring bytes as utf8
pub fn astring_utf8(i: &[u8]) -> IResult<&[u8], &str> {
    map_res(astring, from_utf8)(i)
}

// ASTRING-CHAR = ATOM-CHAR / resp-specials
pub fn is_astring_char(c: u8) -> bool {
    is_atom_char(c) || is_resp_specials(c)
}

// ATOM-CHAR = <any CHAR except atom-specials>
pub fn is_atom_char(c: u8) -> bool {
    is_char(c) && !is_atom_specials(c)
}

// atom-specials = "(" / ")" / "{" / SP / CTL / list-wildcards / quoted-specials / resp-specials
pub fn is_atom_specials(c: u8) -> bool {
    c == b'('
        || c == b')'
        || c == b'{'
        || c == b' '
        || c < 32
        || is_list_wildcards(c)
        || is_quoted_specials(c)
        || is_resp_specials(c)
}

// resp-specials = "]"
pub fn is_resp_specials(c: u8) -> bool {
    c == b']'
}

// atom = 1*ATOM-CHAR
pub fn atom(i: &[u8]) -> IResult<&[u8], &str> {
    map_res(take_while1(is_atom_char), from_utf8)(i)
}

// ----- nstring ----- nil or string

// nstring = string / nil
pub fn nstring(i: &[u8]) -> IResult<&[u8], Option<&[u8]>> {
    alt((map(nil, |_| None), map(string, Some)))(i)
}

// nstring bytes as utf8
pub fn nstring_utf8(i: &[u8]) -> IResult<&[u8], Option<&str>> {
    alt((map(nil, |_| None), map(string_utf8, Some)))(i)
}

// nil = "NIL"
pub fn nil(i: &[u8]) -> IResult<&[u8], &[u8]> {
    tag_no_case("NIL")(i)
}

// ----- text -----

// text = 1*TEXT-CHAR
pub fn text(i: &[u8]) -> IResult<&[u8], &str> {
    map_res(take_while(is_text_char), from_utf8)(i)
}

// TEXT-CHAR = <any CHAR except CR and LF>
pub fn is_text_char(c: u8) -> bool {
    is_char(c) && c != b'\r' && c != b'\n'
}

// CHAR = %x01-7F
//          ; any 7-bit US-ASCII character,
//          ;  excluding NUL
// From RFC5234
pub fn is_char(c: u8) -> bool {
    match c {
        0x01..=0x7F => true,
        _ => false,
    }
}

// ----- others -----

// list-wildcards = "%" / "*"
pub fn is_list_wildcards(c: u8) -> bool {
    c == b'%' || c == b'*'
}

pub fn paren_delimited<'a, F, O, E>(f: F) -> impl Fn(&'a [u8]) -> IResult<&'a [u8], O, E>
where
    F: Fn(&'a [u8]) -> IResult<&'a [u8], O, E>,
    E: nom::error::ParseError<&'a [u8]>,
{
    delimited(char('('), f, char(')'))
}

pub fn parenthesized_nonempty_list<'a, F, O, E>(
    f: F,
) -> impl Fn(&'a [u8]) -> IResult<&'a [u8], Vec<O>, E>
where
    F: Fn(&'a [u8]) -> IResult<&'a [u8], O, E>,
    E: nom::error::ParseError<&'a [u8]>,
{
    delimited(char('('), separated_nonempty_list(char(' '), f), char(')'))
}

pub fn parenthesized_list<'a, F, O, E>(f: F) -> impl Fn(&'a [u8]) -> IResult<&'a [u8], Vec<O>, E>
where
    F: Fn(&'a [u8]) -> IResult<&'a [u8], O, E>,
    E: nom::error::ParseError<&'a [u8]>,
{
    delimited(char('('), separated_list(char(' '), f), char(')'))
}

pub fn opt_opt<'a, F, O, E>(f: F) -> impl Fn(&'a [u8]) -> IResult<&'a [u8], Option<O>, E>
where
    F: Fn(&'a [u8]) -> IResult<&'a [u8], Option<O>, E>,
{
    move |i: &[u8]| match f(i) {
        Ok((i, o)) => Ok((i, o)),
        Err(nom::Err::Error(_)) => Ok((i, None)),
        Err(e) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_string_literal() {
        match string(b"{3}\r\nXYZ") {
            Ok((_, value)) => {
                assert_eq!(value, b"XYZ");
            }
            rsp => panic!("unexpected response {:?}", rsp),
        }
    }

    #[test]
    fn test_astring() {
        match astring(b"text ") {
            Ok((_, value)) => {
                assert_eq!(value, b"text");
            }
            rsp => panic!("unexpected response {:?}", rsp),
        }
    }
}
