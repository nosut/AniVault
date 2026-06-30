use base64::{engine::general_purpose::STANDARD, Engine as _};
use windows::core::PCWSTR;
use windows::Win32::Foundation::{LocalFree, HLOCAL};
use windows::Win32::Security::Cryptography::{
    CryptProtectData, CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
};

pub fn protect_secret(plaintext: &str) -> anyhow::Result<String> {
    let input_bytes = plaintext.as_bytes();
    let input = CRYPT_INTEGER_BLOB {
        cbData: input_bytes.len() as u32,
        pbData: input_bytes.as_ptr() as *mut u8,
    };
    let mut output = CRYPT_INTEGER_BLOB::default();

    unsafe {
        CryptProtectData(
            &input,
            PCWSTR::null(),
            None,
            None,
            None,
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        )?;

        let bytes = std::slice::from_raw_parts(output.pbData, output.cbData as usize);
        let encoded = STANDARD.encode(bytes);
        LocalFree(Some(HLOCAL(output.pbData.cast())));
        Ok(encoded)
    }
}

pub fn unprotect_secret(ciphertext_b64: &str) -> anyhow::Result<String> {
    let mut encrypted = STANDARD.decode(ciphertext_b64)?;
    let input = CRYPT_INTEGER_BLOB {
        cbData: encrypted.len() as u32,
        pbData: encrypted.as_mut_ptr(),
    };
    let mut output = CRYPT_INTEGER_BLOB::default();

    unsafe {
        CryptUnprotectData(&input, None, None, None, None, CRYPTPROTECT_UI_FORBIDDEN, &mut output)?;

        let bytes = std::slice::from_raw_parts(output.pbData, output.cbData as usize);
        let plaintext = String::from_utf8(bytes.to_vec())?;
        LocalFree(Some(HLOCAL(output.pbData.cast())));
        Ok(plaintext)
    }
}
