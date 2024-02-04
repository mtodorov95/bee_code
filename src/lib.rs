/*
Copyright 2024 Mario Todorov.

Permission is hereby granted, free of charge, to any person obtaining
a copy of this software and associated documentation files (the “Software”),
to deal in the Software without restriction, including without limitation
the rights to use, copy, modify, merge, publish, distribute, sublicense,
and/or sell copies of the Software, and to permit persons to whom the
Software is furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included
in all copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED “AS IS”, WITHOUT WARRANTY OF ANY KIND, 
EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF 
MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT.
IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY
CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT,
TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE
OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.
*/

//! 'bee_code' is a library providing methods for encoding and decoding
//! bencoded data - a format used in .torrent files
//! and communication with trackers.
use std::collections::BTreeMap;

/// Custom error types returned during parsing
#[derive(Debug, PartialEq, Eq)]
pub enum BencodeError {
    /// Returned when the number specifying the length of a string
    /// is negative - b"-3:dog"
    /// Includes the position in the vector at which the error occured.
    NegativeLen(String),
    /// Returned when an unexpected byte was found at the current
    /// position during parsing - missing 'e' at the end of list.
    /// Includes the position in the vector at which the error occured.
    Unexpected(String),
    /// Returned when the parsed bytes are not UTF-8.
    /// Includes the position in the vector at which the error occured.
    Utf8Error(String),
}

/// Represent the four types included in the Bencode specification
#[derive(Debug, PartialEq, Eq)]
pub enum Bencode {
    Bytes(Vec<u8>),
    Integer(i64),
    List(Vec<Self>),
    Dict(BTreeMap<Vec<u8>, Self>),
}

impl Bencode {
    /// Parses a bytes vector into Bencode type
    ///
    /// # Errors
    ///
    /// This function will return an error if the input data
    /// doesn't follow the bencode format specification.
    ///
    /// # Examples
    ///
    /// ```
    /// use bee_code::Bencode;
    ///
    /// let res = Bencode::parse(b"i36e".to_vec());
    ///
    /// assert_eq!(
    ///     res,
    ///     Ok(Bencode::Integer(36))
    /// );
    /// ```
    pub fn parse(source: Vec<u8>) -> Result<Self, BencodeError> {
        return Parser::new(&source).decode();
    }

    /// Serializes Bencode types to a bytes vector
    ///
    /// # Examples
    ///
    /// ```
    /// use bee_code::Bencode;
    ///
    /// let int = Bencode::Integer(13);
    ///
    /// assert_eq!(
    ///     int.serialize(),
    ///     vec![105, 49, 51, 101]
    /// );
    /// ```
    pub fn serialize(&self) -> Vec<u8> {
        match self {
            Bencode::Integer(num) => {
                return format!("i{}e", num).as_bytes().to_vec();
            }
            Bencode::List(list) => {
                let mut temp = b"l".to_vec();
                for item in list {
                    temp.extend(item.serialize());
                }
                temp.push(b'e');
                return temp;
            }
            Bencode::Dict(dict) => {
                let mut temp = b"d".to_vec();
                for (key, value) in dict {
                    temp.extend(Self::serialize_bytes(key));
                    temp.extend(value.serialize());
                }
                temp.push(b'e');
                return temp;
            }
            Bencode::Bytes(bytes) => {
                return Self::serialize_bytes(bytes);
            }
        }
    }

    fn serialize_bytes(bytes: &[u8]) -> Vec<u8> {
        let temp = format!("{}:", bytes.len());
        let mut temp = temp.as_bytes().to_vec();
        temp.extend(bytes);
        return temp;
    }
}

struct Parser<'a> {
    pos: usize,
    input: &'a [u8],
}

impl Parser<'_> {
    fn new(bytes: &[u8]) -> Parser {
        return Parser {
            input: bytes,
            pos: 0,
        };
    }

    fn decode(&mut self) -> Result<Bencode, BencodeError> {
        return self.parse_element();
    }

    fn next(&self) -> u8 {
        return self.input.get(self.pos).unwrap().clone();
    }

    fn eof(&self) -> bool {
        return self.pos >= self.input.len();
    }

    fn consume(&mut self) -> u8 {
        let c = self.input.get(self.pos);
        self.pos += 1;
        return c.unwrap().clone();
    }

    fn consume_while<F>(&mut self, test: F) -> Vec<u8>
    where
        F: Fn(u8) -> bool,
    {
        let mut res = vec![];
        while !self.eof() && test(self.next()) {
            res.push(self.consume());
        }
        return res;
    }

    fn consume_expected(&mut self, expected: u8) -> Result<u8, BencodeError> {
        match self.next() {
            c if c == expected => Ok(self.consume()),
            _ => Err(BencodeError::Unexpected(format!(
                "Unexpected character at index {}. Expected {} found {}",
                self.pos,
                expected,
                self.input[self.pos + 1]
            ))),
        }
    }

    fn parse_dict(&mut self) -> Result<Bencode, BencodeError> {
        self.consume_expected(b'd')?;
        let mut dict = BTreeMap::new();

        while self.next() != b'e' {
            let k = self.parse_string()?;
            let v = self.parse_element()?;
            dict.insert(k, v);
        }
        self.consume_expected(b'e')?;
        return Ok(Bencode::Dict(dict));
    }

    fn parse_list(&mut self) -> Result<Bencode, BencodeError> {
        self.consume_expected(b'l')?;
        let mut list = vec![];
        while self.next() != b'e' {
            list.push(self.parse_element()?);
        }
        self.consume_expected(b'e')?;
        return Ok(Bencode::List(list));
    }

    fn parse_element(&mut self) -> Result<Bencode, BencodeError> {
        match self.next() {
            b'd' => self.parse_dict(),
            b'l' => self.parse_list(),
            b'i' => self.parse_int(),
            b'0'..=b'9' => Ok(Bencode::Bytes(self.parse_string()?)),
            _ => Err(BencodeError::Unexpected(format!(
                "Unexpected value type at index {}",
                self.pos
            ))),
        }
    }

    fn parse_int(&mut self) -> Result<Bencode, BencodeError> {
        let pos = self.pos;
        self.consume_expected(b'i')?;
        let mut sign = 1;
        match self.consume_expected(b'-') {
            Ok(_) => sign = -1,
            Err(_) => {}
        }
        let v = self.consume_while(|c| c != b'e');
        if v.len() > 1 && v[0] == b'0' {
            return Err(BencodeError::Unexpected(format!(
                "Leading 0 while parsing integer at index {}",
                pos
            )));
        }
        if v.len() == 1 && v[0] == b'0' && sign == -1 {
            return Err(BencodeError::Unexpected(format!(
                "Negative 0 while parsing integer at index {}",
                pos
            )));
        }
        let int: i64 = match std::str::from_utf8(&v) {
            Ok(value) => value
                .parse()
                .expect("Integer should only include numeric values"),
            Err(e) => {
                return Err(BencodeError::Utf8Error(format!(
                    "Non UTF8 encoded integer value at index {}. {}",
                    pos, e
                )))
            }
        };
        self.consume_expected(b'e')?;
        return Ok(Bencode::Integer(int * sign));
    }

    fn parse_string(&mut self) -> Result<Vec<u8>, BencodeError> {
        let len = self.parse_len()?;
        self.consume_expected(b':')?;
        let mut bytes = vec![];
        for _ in 0..len {
            bytes.push(self.consume());
        }
        return Ok(bytes);
    }

    fn parse_len(&mut self) -> Result<usize, BencodeError> {
        if self.next() == b'-' {
            return Err(BencodeError::NegativeLen(format!(
                "Negative string len at index {}",
                self.pos,
            )));
        }
        let v = self.consume_while(|c| c != b':');
        let len: usize = match std::str::from_utf8(&v) {
            Ok(value) => value
                .parse()
                .expect("String length should include only numbers"),
            Err(e) => {
                return Err(BencodeError::Utf8Error(format!(
                    "Non UTF8 encoded string length at index {}. {}",
                    self.pos, e
                )))
            }
        };
        return Ok(len);
    }
}

#[cfg(test)]
mod test {
    use std::collections::BTreeMap;

    use crate::{Bencode, Parser};

    #[test]
    fn test_parse_string() {
        let mut p = Parser::new(b"6:string");
        assert_eq!(p.parse_string(), Ok(b"string".to_vec()));
    }
    #[test]
    fn test_parse_string_empty() {
        let mut p = Parser::new(b"0:");
        assert_eq!(p.parse_string(), Ok(b"".to_vec()));
    }
    #[test]
    fn test_parse_string_with_neg_len() {
        let mut p = Parser::new(b"-2:text");
        assert_eq!(
            p.parse_string(),
            Err(crate::BencodeError::NegativeLen(
                "Negative string len at index 0".to_owned(),
            ))
        );
    }
    #[test]
    fn test_parse_int() {
        let mut p = Parser::new(b"i13e");
        assert_eq!(p.parse_int(), Ok(Bencode::Integer(13)));
    }
    #[test]
    fn test_parse_int_neg() {
        let mut p = Parser::new(b"i-13e");
        assert_eq!(p.parse_int(), Ok(Bencode::Integer(-13)));
    }
    #[test]
    fn test_parse_int_neg_zero() {
        let mut p = Parser::new(b"i-0e");
        assert_eq!(
            p.parse_int(),
            Err(crate::BencodeError::Unexpected(
                "Negative 0 while parsing integer at index 0".to_owned()
            ))
        );
    }
    #[test]
    fn test_parse_int_lead_zero() {
        let mut p = Parser::new(b"i0934e");
        assert_eq!(
            p.parse_int(),
            Err(crate::BencodeError::Unexpected(
                "Leading 0 while parsing integer at index 0".to_owned()
            ))
        );
    }

    #[test]
    fn test_parse_list() {
        let mut p = Parser::new(b"l4:spam3:doge");
        assert_eq!(
            p.parse_list(),
            Ok(Bencode::List(vec![
                Bencode::Bytes(b"spam".to_vec()),
                Bencode::Bytes(b"dog".to_vec()),
            ]))
        );
    }
    #[test]
    fn test_parse_list_empty() {
        let mut p = Parser::new(b"le");
        assert_eq!(p.parse_list(), Ok(Bencode::List(vec![])));
    }
    #[test]
    fn test_parse_dict() {
        let mut p = Parser::new(b"d4:spam3:dog3:cati36ee");
        assert_eq!(
            p.parse_dict(),
            Ok(Bencode::Dict(BTreeMap::from([
                (b"spam".to_vec(), Bencode::Bytes(b"dog".to_vec())),
                (b"cat".to_vec(), Bencode::Integer(36)),
            ])))
        );
    }

    #[test]
    fn test_parse_dict_empty() {
        let mut p = Parser::new(b"de");
        assert_eq!(p.parse_dict(), Ok(Bencode::Dict(BTreeMap::new())));
    }
}
