use crate::errors::LedgerError;

const MAX_DATA_SIZE: usize = 255;

/// APDU data blob, limited to 255 bytes. For simplicity, this data does not support 3-byte APDU
/// prefixes.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct APDUData(Vec<u8>);

impl APDUData {
    /// Instantiate a APDUData from a slice. If the slice contains more than 255 bytes, only the
    /// first 255 are used.
    pub fn new(buf: &[u8]) -> Self {
        let length = std::cmp::min(buf.len(), MAX_DATA_SIZE);
        APDUData(buf[..length].to_vec())
    }

    /// Return the data length in bytes
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// True if the underlying slice is empty, else false.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Resize the data, as a vec.
    pub fn resize(&mut self, new_size: usize, fill_with: u8) {
        self.0
            .resize(std::cmp::min(new_size, MAX_DATA_SIZE), fill_with)
    }

    /// Consume the struct and get the underlying data
    pub fn data(self) -> Vec<u8> {
        self.0
    }
}

impl From<&[u8]> for APDUData {
    fn from(buf: &[u8]) -> Self {
        Self::new(buf)
    }
}

impl From<Vec<u8>> for APDUData {
    fn from(mut v: Vec<u8>) -> Self {
        v.resize(std::cmp::min(v.len(), MAX_DATA_SIZE), 0);
        Self(v)
    }
}

impl AsRef<[u8]> for APDUData {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        &self.0[..]
    }
}

/// An APDU Command packet, used to issue instructions to the smart card.
/// See [wikipedia](https://en.wikipedia.org/wiki/Smart_card_application_protocol_data_unit) for
/// additional format details
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct APDUCommand {
    /// The instruction code.
    pub ins: u8,
    /// Instruction parameter 1
    pub p1: u8,
    /// Instruction parameter 2
    pub p2: u8,
    /// Command data
    pub data: APDUData,
    /// The requested response length.
    pub response_len: Option<u8>,
}

impl APDUCommand {
    /// Return the serialized length of the APDU packet
    pub fn serialized_length(&self) -> usize {
        let mut length = 4;
        if !self.data.is_empty() {
            length += 1;
            length += self.data.len();
        }
        length += if self.response_len.is_some() { 1 } else { 0 };
        length
    }

    /// Write the APDU packet to the specified Write interface
    pub fn write_to<W: std::io::Write>(&self, w: &mut W) -> Result<usize, std::io::Error> {
        w.write_all(&[0xE0, self.ins, self.p1, self.p2])?;
        if !self.data.is_empty() {
            w.write_all(&[self.data.len() as u8])?;
            w.write_all(&self.data.as_ref())?;
        }
        if let Some(response_len) = self.response_len {
            w.write_all(&[response_len])?;
        }
        Ok(self.serialized_length())
    }

    /// Serialize the APDU to a vector
    pub fn serialize(&self) -> Vec<u8> {
        let mut v = Vec::with_capacity(self.serialized_length());
        self.write_to(&mut v).unwrap();
        v
    }
}

/// An APDU response is a wrapper around some response bytes. To avoid unnecessary clones, it
/// exposes the retcode and response data as getters.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct APDUAnswer {
    response: Vec<u8>,
}

impl std::fmt::Display for APDUAnswer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "APDUAnswer: {{\n\tResponse: {} \n\tData: {:?}\n}}",
            self.response_status(),
            self.data()
        )
    }
}

impl APDUAnswer {
    /// instantiate a
    pub fn from_answer(response: Vec<u8>) -> Result<APDUAnswer, LedgerError> {
        if response.len() < 2 {
            Err(LedgerError::ResponseTooShort(response.to_vec()))
        } else {
            Ok(Self { response })
        }
    }

    /// Return the response length in bytes
    pub fn len(&self) -> usize {
        self.response.len()
    }

    /// True if the underlying slice is empty, else false.
    pub fn is_empty(&self) -> bool {
        self.response.is_empty()
    }

    /// Return false if the response status is an error.
    pub fn is_success(&self) -> bool {
        self.response_status().is_success()
    }

    /// Get the integer response code from the response packet.
    ///
    /// Panics if the buffer is too short (some device error).
    pub fn retcode(&self) -> u16 {
        let mut buf = [0u8; 2];
        buf.copy_from_slice(&self.response[self.len() - 2..]);
        u16::from_be_bytes(buf)
    }

    /// Return the Response code
    ///
    /// Panics on unknown retcode.
    pub fn response_status(&self) -> APDUResponseCodes {
        self.retcode().into()
    }

    /// Return a reference to the response data, or None if the response errored
    pub fn data(&self) -> Option<&[u8]> {
        if self.is_success() {
            Some(&self.response[..self.len() - 2])
        } else {
            None
        }
    }
}

#[repr(u16)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
/// APDU Response codes. These are the last 2 bytes of the APDU packet. Please see APDU and
/// Ledger documentation for each error type.
pub enum APDUResponseCodes {
    /// No Error
    NoError = 0x9000,
    /// ExecutionError
    ExecutionError = 0x6400,
    /// WrongLength
    WrongLength = 0x6700,
    /// EmptyBuffer
    EmptyBuffer = 0x6982,
    /// OutputBufferTooSmall
    OutputBufferTooSmall = 0x6983,
    /// DataInvalid
    DataInvalid = 0x6984,
    /// ConditionsNotSatisfied
    ConditionsNotSatisfied = 0x6985,
    /// CommandNotAllowed
    CommandNotAllowed = 0x6986,
    /// BadKeyHandle
    BadKeyHandle = 0x6A80,
    /// InvalidP1P2
    InvalidP1P2 = 0x6B00,
    /// InsNotSupported
    InsNotSupported = 0x6D00,
    /// ClaNotSupported
    ClaNotSupported = 0x6E00,
    /// Unknown
    Unknown = 0x6F00,
    /// SignVerifyError
    SignVerifyError = 0x6F01,
}

impl std::fmt::Display for APDUResponseCodes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Code {:x} ({})", *self as u16, self.description())
    }
}

impl APDUResponseCodes {
    /// True if the response is a success, else false.
    pub fn is_success(self) -> bool {
        self == APDUResponseCodes::NoError
    }

    /// Return a description of the response code.
    pub fn description(self) -> &'static str {
        match self {
            APDUResponseCodes::NoError => "[APDU_CODE_NOERROR]",
            APDUResponseCodes::ExecutionError => {
                "[APDU_CODE_EXECUTION_ERROR] No information given (NV-Ram not changed)"
            }
            APDUResponseCodes::WrongLength => "[APDU_CODE_WRONG_LENGTH] Wrong length",
            APDUResponseCodes::EmptyBuffer => "[APDU_CODE_EMPTY_BUFFER]",
            APDUResponseCodes::OutputBufferTooSmall => "[APDU_CODE_OUTPUT_BUFFER_TOO_SMALL]",
            APDUResponseCodes::DataInvalid => {
                "[APDU_CODE_DATA_INVALID] data reversibly blocked (invalidated)"
            }
            APDUResponseCodes::ConditionsNotSatisfied => {
                "[APDU_CODE_CONDITIONS_NOT_SATISFIED] Conditions of use not satisfied"
            }
            APDUResponseCodes::CommandNotAllowed => {
                "[APDU_CODE_COMMAND_NOT_ALLOWED] Command not allowed (no current EF)"
            }
            APDUResponseCodes::BadKeyHandle => {
                "[APDU_CODE_BAD_KEY_HANDLE] The parameters in the data field are incorrect"
            }
            APDUResponseCodes::InvalidP1P2 => "[APDU_CODE_INVALIDP1P2] Wrong parameter(s) P1-P2",
            APDUResponseCodes::InsNotSupported => {
                "[APDU_CODE_INS_NOT_SUPPORTED] Instruction code not supported or invalid"
            }
            APDUResponseCodes::ClaNotSupported => {
                "[APDU_CODE_CLA_NOT_SUPPORTED] Class not supported"
            }
            APDUResponseCodes::Unknown => "[APDU_CODE_UNKNOWN]",
            APDUResponseCodes::SignVerifyError => "[APDU_CODE_SIGN_VERIFY_ERROR]",
        }
    }
}

impl From<u16> for APDUResponseCodes {
    fn from(code: u16) -> Self {
        match code {
            0x9000 => APDUResponseCodes::NoError,
            0x6400 => APDUResponseCodes::ExecutionError,
            0x6700 => APDUResponseCodes::WrongLength,
            0x6982 => APDUResponseCodes::EmptyBuffer,
            0x6983 => APDUResponseCodes::OutputBufferTooSmall,
            0x6984 => APDUResponseCodes::DataInvalid,
            0x6985 => APDUResponseCodes::ConditionsNotSatisfied,
            0x6986 => APDUResponseCodes::CommandNotAllowed,
            0x6A80 => APDUResponseCodes::BadKeyHandle,
            0x6B00 => APDUResponseCodes::InvalidP1P2,
            0x6D00 => APDUResponseCodes::InsNotSupported,
            0x6E00 => APDUResponseCodes::ClaNotSupported,
            0x6F00 => APDUResponseCodes::Unknown,
            0x6F01 => APDUResponseCodes::SignVerifyError,
            _ => {
                panic!("Unknown APDU response code {:x}", code)
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn serialize() {
        let data: &[u8] = &[0, 0, 0, 1, 0, 0, 0, 1];

        let command = APDUCommand {
            ins: 0x01,
            p1: 0x00,
            p2: 0x00,
            data: data.into(),
            response_len: None,
        };

        let serialized_command = command.serialize();
        let expected = vec![224, 1, 0, 0, 8, 0, 0, 0, 1, 0, 0, 0, 1];
        assert_eq!(serialized_command, expected);

        let command = APDUCommand {
            ins: 0x01,
            p1: 0x00,
            p2: 0x00,
            data: data.into(),
            response_len: Some(13),
        };

        let serialized_command = command.serialize();
        let expected = vec![224, 1, 0, 0, 8, 0, 0, 0, 1, 0, 0, 0, 1, 13];
        assert_eq!(serialized_command, expected)
    }
}

/*******************************************************************************
*   (c) 2020 ZondaX GmbH
*
*  Licensed under the Apache License, Version 2.0 (the "License");
*  you may not use this file except in compliance with the License.
*  You may obtain a copy of the License at
*
*      http://www.apache.org/licenses/LICENSE-2.0
*
*  Unless required by applicable law or agreed to in writing, software
*  distributed under the License is distributed on an "AS IS" BASIS,
*  WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
*  See the License for the specific language governing permissions and
*  limitations under the License.
********************************************************************************/
