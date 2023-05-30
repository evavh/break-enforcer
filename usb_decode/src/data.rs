use crate::Error;


pub struct Data {
    // data is left encoded. To access it as bytes
    // we provide a member function (todo)

    // encoded: &'a [u8]
}

pub(crate) fn parse<'a>(bits: &'a [u8]) -> Result<(&[u8], Data), Error> {

    // todo hard, will need to move forward till EOP to
    // find the length then check CRC

    Ok((bits, Data{}))
}
