extern crate alloc;

use alloc::vec;
use alloc::vec::Vec;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mp4Error {
    Truncated,
    InvalidBoxSize,
    IntegerOverflow,
    MissingMoov,
    DuplicateMoov,
    FragmentedMp4,
    MissingVideoTrack,
    MultipleVideoTracks,
    MultipleAudioTracks,
    UnsupportedTrack,
    UnsupportedEditList,
    ExternalDataReference,
    MissingMediaHeader,
    MissingSampleTable,
    MissingTable(&'static str),
    UnsupportedCodec([u8; 4]),
    InvalidTable(&'static str),
    SampleCountMismatch,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VideoSample {
    pub index: u32,
    pub decode_time: u64,
    pub presentation_time: i64,
    pub duration: u32,
    pub byte_offset: u64,
    pub byte_size: u32,
    pub is_sync: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VideoTrack {
    pub timescale: u32,
    pub media_duration: u64,
    pub presentation_start: i64,
    pub presentation_end: i64,
    pub codec: [u8; 4],
    pub samples: Vec<VideoSample>,
}

#[derive(Clone, Copy)]
struct IsoBox<'a> {
    kind: [u8; 4],
    payload: &'a [u8],
}

struct BoxIter<'a> {
    data: &'a [u8],
    position: usize,
}

impl<'a> BoxIter<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, position: 0 }
    }
}

impl<'a> Iterator for BoxIter<'a> {
    type Item = Result<IsoBox<'a>, Mp4Error>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.position == self.data.len() {
            return None;
        }
        let remaining = &self.data[self.position..];
        if remaining.len() < 8 {
            self.position = self.data.len();
            return Some(Err(Mp4Error::Truncated));
        }

        let size32 = read_u32(remaining, 0).expect("checked 8-byte box header");
        let kind = [remaining[4], remaining[5], remaining[6], remaining[7]];
        let (header_size, box_size) = if size32 == 1 {
            if remaining.len() < 16 {
                self.position = self.data.len();
                return Some(Err(Mp4Error::Truncated));
            }
            let size64 = read_u64(remaining, 8).expect("checked extended box header");
            (
                16usize,
                match usize::try_from(size64) {
                    Ok(size) => size,
                    Err(_) => {
                        self.position = self.data.len();
                        return Some(Err(Mp4Error::IntegerOverflow));
                    }
                },
            )
        } else if size32 == 0 {
            (8usize, remaining.len())
        } else {
            (8usize, size32 as usize)
        };

        if box_size < header_size || box_size > remaining.len() {
            self.position = self.data.len();
            return Some(Err(Mp4Error::InvalidBoxSize));
        }
        self.position = match self.position.checked_add(box_size) {
            Some(position) => position,
            None => {
                self.position = self.data.len();
                return Some(Err(Mp4Error::IntegerOverflow));
            }
        };
        Some(Ok(IsoBox {
            kind,
            payload: &remaining[header_size..box_size],
        }))
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Handler {
    Video,
    Audio,
}

struct TrackTables<'a> {
    timescale: u32,
    media_duration: u64,
    edit_media_time: i64,
    stsd: &'a [u8],
    stts: &'a [u8],
    ctts: Option<&'a [u8]>,
    stss: Option<&'a [u8]>,
    stsz: &'a [u8],
    stsc: &'a [u8],
    chunk_offsets: &'a [u8],
    chunk_offsets_are_64_bit: bool,
}

pub fn parse_mp4_video_track(data: &[u8]) -> Result<VideoTrack, Mp4Error> {
    let mut moov = None;
    for item in BoxIter::new(data) {
        let item = item?;
        match &item.kind {
            b"moov" => {
                if moov.replace(item.payload).is_some() {
                    return Err(Mp4Error::DuplicateMoov);
                }
            }
            b"moof" => return Err(Mp4Error::FragmentedMp4),
            _ => {}
        }
    }
    let moov = moov.ok_or(Mp4Error::MissingMoov)?;

    let mut video = None;
    let mut audio_tracks = 0usize;
    for item in BoxIter::new(moov) {
        let item = item?;
        if item.kind != *b"trak" {
            continue;
        }
        match parse_track(item.payload)? {
            (Handler::Video, tables) => {
                if video.is_some() {
                    return Err(Mp4Error::MultipleVideoTracks);
                }
                video = Some(build_video_track(tables)?);
            }
            (Handler::Audio, _) => {
                audio_tracks += 1;
                if audio_tracks > 1 {
                    return Err(Mp4Error::MultipleAudioTracks);
                }
            }
        }
    }
    let video = video.ok_or(Mp4Error::MissingVideoTrack)?;
    let file_len = data.len() as u64;
    for sample in &video.samples {
        let end = sample
            .byte_offset
            .checked_add(sample.byte_size as u64)
            .ok_or(Mp4Error::IntegerOverflow)?;
        if end > file_len {
            return Err(Mp4Error::InvalidTable("sample byte range"));
        }
    }
    Ok(video)
}

fn parse_track(track: &[u8]) -> Result<(Handler, TrackTables<'_>), Mp4Error> {
    let edit_media_time = parse_edit_media_time(track)?;
    let mdia = find_child(track, *b"mdia")?.ok_or(Mp4Error::MissingMediaHeader)?;
    let hdlr = find_child(mdia, *b"hdlr")?.ok_or(Mp4Error::MissingMediaHeader)?;
    let handler = parse_handler(hdlr)?;
    let mdhd = find_child(mdia, *b"mdhd")?.ok_or(Mp4Error::MissingMediaHeader)?;
    let (timescale, media_duration) = parse_mdhd(mdhd)?;
    let minf = find_child(mdia, *b"minf")?.ok_or(Mp4Error::MissingSampleTable)?;
    verify_self_contained(minf)?;
    let stbl = find_child(minf, *b"stbl")?.ok_or(Mp4Error::MissingSampleTable)?;

    let tables = TrackTables {
        timescale,
        media_duration,
        edit_media_time,
        stsd: required_child(stbl, *b"stsd", "stsd")?,
        stts: required_child(stbl, *b"stts", "stts")?,
        ctts: find_child(stbl, *b"ctts")?,
        stss: find_child(stbl, *b"stss")?,
        stsz: required_child(stbl, *b"stsz", "stsz")?,
        stsc: required_child(stbl, *b"stsc", "stsc")?,
        chunk_offsets: if let Some(stco) = find_child(stbl, *b"stco")? {
            stco
        } else {
            required_child(stbl, *b"co64", "stco/co64")?
        },
        chunk_offsets_are_64_bit: find_child(stbl, *b"stco")?.is_none(),
    };
    Ok((handler, tables))
}

fn parse_edit_media_time(track: &[u8]) -> Result<i64, Mp4Error> {
    let Some(edts) = find_child(track, *b"edts")? else {
        return Ok(0);
    };
    let elst = find_child(edts, *b"elst")?.ok_or(Mp4Error::UnsupportedEditList)?;
    let version = *elst.first().ok_or(Mp4Error::Truncated)?;
    let entry_count = read_u32(elst, 4).ok_or(Mp4Error::Truncated)?;
    if entry_count != 1 {
        return Err(Mp4Error::UnsupportedEditList);
    }

    let (segment_duration, media_time, rate_offset) = match version {
        0 => (
            read_u32(elst, 8).ok_or(Mp4Error::Truncated)? as u64,
            read_u32(elst, 12).ok_or(Mp4Error::Truncated)? as i32 as i64,
            16,
        ),
        1 => (
            read_u64(elst, 8).ok_or(Mp4Error::Truncated)?,
            read_u64(elst, 16).ok_or(Mp4Error::Truncated)? as i64,
            24,
        ),
        _ => return Err(Mp4Error::UnsupportedEditList),
    };
    let rate_integer = read_i16(elst, rate_offset).ok_or(Mp4Error::Truncated)?;
    let rate_fraction = read_i16(elst, rate_offset + 2).ok_or(Mp4Error::Truncated)?;
    if segment_duration == 0 || media_time < 0 || rate_integer != 1 || rate_fraction != 0 {
        return Err(Mp4Error::UnsupportedEditList);
    }
    Ok(media_time)
}

fn parse_handler(data: &[u8]) -> Result<Handler, Mp4Error> {
    let handler = read_fourcc(data, 8).ok_or(Mp4Error::Truncated)?;
    match &handler {
        b"vide" => Ok(Handler::Video),
        b"soun" => Ok(Handler::Audio),
        _ => Err(Mp4Error::UnsupportedTrack),
    }
}

fn parse_mdhd(data: &[u8]) -> Result<(u32, u64), Mp4Error> {
    let version = *data.first().ok_or(Mp4Error::Truncated)?;
    let (timescale, duration) = match version {
        0 => (
            read_u32(data, 12).ok_or(Mp4Error::Truncated)?,
            read_u32(data, 16).ok_or(Mp4Error::Truncated)? as u64,
        ),
        1 => (
            read_u32(data, 20).ok_or(Mp4Error::Truncated)?,
            read_u64(data, 24).ok_or(Mp4Error::Truncated)?,
        ),
        _ => return Err(Mp4Error::InvalidTable("mdhd version")),
    };
    if timescale == 0 {
        return Err(Mp4Error::InvalidTable("zero timescale"));
    }
    Ok((timescale, duration))
}

fn verify_self_contained(minf: &[u8]) -> Result<(), Mp4Error> {
    let Some(dinf) = find_child(minf, *b"dinf")? else {
        return Ok(());
    };
    let Some(dref) = find_child(dinf, *b"dref")? else {
        return Err(Mp4Error::ExternalDataReference);
    };
    let entry_count = read_u32(dref, 4).ok_or(Mp4Error::Truncated)? as usize;
    let entries = dref.get(8..).ok_or(Mp4Error::Truncated)?;
    let mut seen = 0usize;
    for item in BoxIter::new(entries) {
        let item = item?;
        seen += 1;
        if item.kind != *b"url " || item.payload.len() < 4 {
            return Err(Mp4Error::ExternalDataReference);
        }
        let flags = ((item.payload[1] as u32) << 16)
            | ((item.payload[2] as u32) << 8)
            | item.payload[3] as u32;
        if flags & 1 == 0 {
            return Err(Mp4Error::ExternalDataReference);
        }
    }
    if seen != entry_count {
        return Err(Mp4Error::InvalidTable("dref entry count"));
    }
    Ok(())
}

fn build_video_track(tables: TrackTables<'_>) -> Result<VideoTrack, Mp4Error> {
    let codec = parse_video_codec(tables.stsd)?;
    let sample_sizes = parse_stsz(tables.stsz)?;
    let sample_count = sample_sizes.len();
    let timings = parse_stts(tables.stts, sample_count)?;
    let composition_offsets = parse_ctts(tables.ctts, sample_count)?;
    let sync = parse_stss(tables.stss, sample_count)?;
    let chunk_offsets = parse_chunk_offsets(tables.chunk_offsets, tables.chunk_offsets_are_64_bit)?;
    let stsc = parse_stsc(tables.stsc)?;
    let byte_offsets = expand_sample_offsets(&sample_sizes, &chunk_offsets, &stsc)?;

    let mut samples = Vec::with_capacity(sample_count);
    let mut decode_time = 0u64;
    let mut presentation_start = i64::MAX;
    let mut presentation_end = i64::MIN;
    for index in 0..sample_count {
        let duration = timings[index];
        let decode_i64 = i64::try_from(decode_time).map_err(|_| Mp4Error::IntegerOverflow)?;
        let presentation_time = decode_i64
            .checked_add(composition_offsets[index])
            .and_then(|time| time.checked_sub(tables.edit_media_time))
            .ok_or(Mp4Error::IntegerOverflow)?;
        let end = presentation_time
            .checked_add(duration as i64)
            .ok_or(Mp4Error::IntegerOverflow)?;
        presentation_start = presentation_start.min(presentation_time);
        presentation_end = presentation_end.max(end);
        samples.push(VideoSample {
            index: index as u32,
            decode_time,
            presentation_time,
            duration,
            byte_offset: byte_offsets[index],
            byte_size: sample_sizes[index],
            is_sync: sync[index],
        });
        decode_time = decode_time
            .checked_add(duration as u64)
            .ok_or(Mp4Error::IntegerOverflow)?;
    }
    if samples.is_empty() {
        return Err(Mp4Error::InvalidTable("empty video track"));
    }
    Ok(VideoTrack {
        timescale: tables.timescale,
        media_duration: tables.media_duration,
        presentation_start,
        presentation_end,
        codec,
        samples,
    })
}

fn parse_video_codec(stsd: &[u8]) -> Result<[u8; 4], Mp4Error> {
    let entry_count = read_u32(stsd, 4).ok_or(Mp4Error::Truncated)?;
    if entry_count != 1 {
        return Err(Mp4Error::InvalidTable("stsd entry count"));
    }
    let entry = stsd.get(8..).ok_or(Mp4Error::Truncated)?;
    if entry.len() < 8 {
        return Err(Mp4Error::Truncated);
    }
    let size = read_u32(entry, 0).ok_or(Mp4Error::Truncated)? as usize;
    if size < 8 || size > entry.len() {
        return Err(Mp4Error::InvalidTable("stsd entry size"));
    }
    let codec = read_fourcc(entry, 4).ok_or(Mp4Error::Truncated)?;
    if codec != *b"avc1" && codec != *b"avc3" {
        return Err(Mp4Error::UnsupportedCodec(codec));
    }
    Ok(codec)
}

fn parse_stsz(data: &[u8]) -> Result<Vec<u32>, Mp4Error> {
    let fixed_size = read_u32(data, 4).ok_or(Mp4Error::Truncated)?;
    let sample_count = read_u32(data, 8).ok_or(Mp4Error::Truncated)? as usize;
    if fixed_size != 0 {
        return Ok(vec![fixed_size; sample_count]);
    }
    let mut sizes = Vec::with_capacity(sample_count);
    for index in 0..sample_count {
        sizes.push(read_u32(data, 12 + index * 4).ok_or(Mp4Error::Truncated)?);
    }
    Ok(sizes)
}

fn parse_stts(data: &[u8], sample_count: usize) -> Result<Vec<u32>, Mp4Error> {
    let entry_count = read_u32(data, 4).ok_or(Mp4Error::Truncated)? as usize;
    let mut durations = Vec::with_capacity(sample_count);
    for index in 0..entry_count {
        let base = 8 + index * 8;
        let count = read_u32(data, base).ok_or(Mp4Error::Truncated)? as usize;
        let delta = read_u32(data, base + 4).ok_or(Mp4Error::Truncated)?;
        if count == 0 || delta == 0 || durations.len().saturating_add(count) > sample_count {
            return Err(Mp4Error::InvalidTable("stts"));
        }
        durations.extend(core::iter::repeat_n(delta, count));
    }
    if durations.len() != sample_count {
        return Err(Mp4Error::SampleCountMismatch);
    }
    Ok(durations)
}

fn parse_ctts(data: Option<&[u8]>, sample_count: usize) -> Result<Vec<i64>, Mp4Error> {
    let Some(data) = data else {
        return Ok(vec![0; sample_count]);
    };
    let version = *data.first().ok_or(Mp4Error::Truncated)?;
    if version > 1 {
        return Err(Mp4Error::InvalidTable("ctts version"));
    }
    let entry_count = read_u32(data, 4).ok_or(Mp4Error::Truncated)? as usize;
    let mut signed_offsets = version == 1;
    for index in 0..entry_count {
        let raw = read_u32(data, 8 + index * 8 + 4).ok_or(Mp4Error::Truncated)?;
        signed_offsets |= raw > i32::MAX as u32;
    }
    let mut offsets = Vec::with_capacity(sample_count);
    for index in 0..entry_count {
        let base = 8 + index * 8;
        let count = read_u32(data, base).ok_or(Mp4Error::Truncated)? as usize;
        let raw = read_u32(data, base + 4).ok_or(Mp4Error::Truncated)?;
        let offset = if signed_offsets {
            (raw as i32) as i64
        } else {
            raw as i64
        };
        if count == 0 || offsets.len().saturating_add(count) > sample_count {
            return Err(Mp4Error::InvalidTable("ctts"));
        }
        offsets.extend(core::iter::repeat_n(offset, count));
    }
    if offsets.len() != sample_count {
        return Err(Mp4Error::SampleCountMismatch);
    }
    Ok(offsets)
}

fn parse_stss(data: Option<&[u8]>, sample_count: usize) -> Result<Vec<bool>, Mp4Error> {
    let Some(data) = data else {
        return Ok(vec![true; sample_count]);
    };
    let entry_count = read_u32(data, 4).ok_or(Mp4Error::Truncated)? as usize;
    let mut sync = vec![false; sample_count];
    let mut previous = 0u32;
    for index in 0..entry_count {
        let sample_number = read_u32(data, 8 + index * 4).ok_or(Mp4Error::Truncated)?;
        if sample_number == 0 || sample_number <= previous || sample_number as usize > sample_count
        {
            return Err(Mp4Error::InvalidTable("stss"));
        }
        sync[sample_number as usize - 1] = true;
        previous = sample_number;
    }
    Ok(sync)
}

#[derive(Clone, Copy)]
struct StscEntry {
    first_chunk: u32,
    samples_per_chunk: u32,
}

fn parse_stsc(data: &[u8]) -> Result<Vec<StscEntry>, Mp4Error> {
    let entry_count = read_u32(data, 4).ok_or(Mp4Error::Truncated)? as usize;
    let mut entries = Vec::with_capacity(entry_count);
    let mut previous = 0u32;
    for index in 0..entry_count {
        let base = 8 + index * 12;
        let first_chunk = read_u32(data, base).ok_or(Mp4Error::Truncated)?;
        let samples_per_chunk = read_u32(data, base + 4).ok_or(Mp4Error::Truncated)?;
        let sample_description = read_u32(data, base + 8).ok_or(Mp4Error::Truncated)?;
        if first_chunk == 0
            || first_chunk <= previous
            || samples_per_chunk == 0
            || sample_description != 1
        {
            return Err(Mp4Error::InvalidTable("stsc"));
        }
        entries.push(StscEntry {
            first_chunk,
            samples_per_chunk,
        });
        previous = first_chunk;
    }
    if entries.first().map(|entry| entry.first_chunk) != Some(1) {
        return Err(Mp4Error::InvalidTable("stsc first chunk"));
    }
    Ok(entries)
}

fn parse_chunk_offsets(data: &[u8], are_64_bit: bool) -> Result<Vec<u64>, Mp4Error> {
    let entry_count = read_u32(data, 4).ok_or(Mp4Error::Truncated)? as usize;
    let width = if are_64_bit { 8 } else { 4 };
    let mut offsets = Vec::with_capacity(entry_count);
    for index in 0..entry_count {
        let position = 8 + index * width;
        offsets.push(if are_64_bit {
            read_u64(data, position).ok_or(Mp4Error::Truncated)?
        } else {
            read_u32(data, position).ok_or(Mp4Error::Truncated)? as u64
        });
    }
    Ok(offsets)
}

fn expand_sample_offsets(
    sample_sizes: &[u32],
    chunk_offsets: &[u64],
    stsc: &[StscEntry],
) -> Result<Vec<u64>, Mp4Error> {
    let mut offsets = Vec::with_capacity(sample_sizes.len());
    let mut sample_index = 0usize;
    let mut stsc_index = 0usize;
    for (chunk_zero_index, chunk_start) in chunk_offsets.iter().copied().enumerate() {
        let chunk_number = chunk_zero_index as u32 + 1;
        while stsc_index + 1 < stsc.len() && stsc[stsc_index + 1].first_chunk <= chunk_number {
            stsc_index += 1;
        }
        let mut offset = chunk_start;
        for _ in 0..stsc[stsc_index].samples_per_chunk {
            let size = *sample_sizes
                .get(sample_index)
                .ok_or(Mp4Error::SampleCountMismatch)?;
            offsets.push(offset);
            offset = offset
                .checked_add(size as u64)
                .ok_or(Mp4Error::IntegerOverflow)?;
            sample_index += 1;
        }
    }
    if sample_index != sample_sizes.len() {
        return Err(Mp4Error::SampleCountMismatch);
    }
    Ok(offsets)
}

fn required_child<'a>(
    data: &'a [u8],
    kind: [u8; 4],
    name: &'static str,
) -> Result<&'a [u8], Mp4Error> {
    find_child(data, kind)?.ok_or(Mp4Error::MissingTable(name))
}

fn find_child(data: &[u8], kind: [u8; 4]) -> Result<Option<&[u8]>, Mp4Error> {
    let mut found = None;
    for item in BoxIter::new(data) {
        let item = item?;
        if item.kind == kind {
            if found.is_some() {
                return Err(Mp4Error::InvalidTable("duplicate box"));
            }
            found = Some(item.payload);
        }
    }
    Ok(found)
}

fn read_u32(data: &[u8], offset: usize) -> Option<u32> {
    Some(u32::from_be_bytes(
        data.get(offset..offset + 4)?.try_into().ok()?,
    ))
}

fn read_u64(data: &[u8], offset: usize) -> Option<u64> {
    Some(u64::from_be_bytes(
        data.get(offset..offset + 8)?.try_into().ok()?,
    ))
}

fn read_i16(data: &[u8], offset: usize) -> Option<i16> {
    Some(i16::from_be_bytes(
        data.get(offset..offset + 2)?.try_into().ok()?,
    ))
}

fn read_fourcc(data: &[u8], offset: usize) -> Option<[u8; 4]> {
    data.get(offset..offset + 4)?.try_into().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn full_box(version: u8, body: &[u8]) -> Vec<u8> {
        let mut data = vec![version, 0, 0, 0];
        data.extend_from_slice(body);
        data
    }

    fn mp4_box(kind: &[u8; 4], payload: &[u8]) -> Vec<u8> {
        let mut data = Vec::with_capacity(payload.len() + 8);
        data.extend_from_slice(&((payload.len() + 8) as u32).to_be_bytes());
        data.extend_from_slice(kind);
        data.extend_from_slice(payload);
        data
    }

    fn concat_boxes(boxes: &[Vec<u8>]) -> Vec<u8> {
        boxes.iter().flatten().copied().collect()
    }

    fn table(entries: &[[u32; 2]]) -> Vec<u8> {
        let mut body = Vec::new();
        body.extend_from_slice(&(entries.len() as u32).to_be_bytes());
        for [a, b] in entries {
            body.extend_from_slice(&a.to_be_bytes());
            body.extend_from_slice(&b.to_be_bytes());
        }
        full_box(0, &body)
    }

    fn synthetic_mp4(codec: [u8; 4]) -> Vec<u8> {
        let mut mdhd_body = vec![0; 8];
        mdhd_body.extend_from_slice(&1_000u32.to_be_bytes());
        mdhd_body.extend_from_slice(&3_000u32.to_be_bytes());
        mdhd_body.extend_from_slice(&[0; 4]);
        let mdhd = mp4_box(b"mdhd", &full_box(0, &mdhd_body));

        let mut hdlr_body = vec![0; 4];
        hdlr_body.extend_from_slice(b"vide");
        hdlr_body.extend_from_slice(&[0; 12]);
        let hdlr = mp4_box(b"hdlr", &full_box(0, &hdlr_body));

        let mut sample_entry = Vec::new();
        sample_entry.extend_from_slice(&8u32.to_be_bytes());
        sample_entry.extend_from_slice(&codec);
        let mut stsd_body = 1u32.to_be_bytes().to_vec();
        stsd_body.extend_from_slice(&sample_entry);
        let stsd = mp4_box(b"stsd", &full_box(0, &stsd_body));
        let stts = mp4_box(b"stts", &table(&[[3, 1_000]]));
        let stss = mp4_box(b"stss", &full_box(0, &[0, 0, 0, 2, 0, 0, 0, 1, 0, 0, 0, 3]));

        let mut stsz_body = Vec::new();
        stsz_body.extend_from_slice(&0u32.to_be_bytes());
        stsz_body.extend_from_slice(&3u32.to_be_bytes());
        for size in [10u32, 20, 30] {
            stsz_body.extend_from_slice(&size.to_be_bytes());
        }
        let stsz = mp4_box(b"stsz", &full_box(0, &stsz_body));

        let mut stsc_body = 1u32.to_be_bytes().to_vec();
        stsc_body.extend_from_slice(&1u32.to_be_bytes());
        stsc_body.extend_from_slice(&3u32.to_be_bytes());
        stsc_body.extend_from_slice(&1u32.to_be_bytes());
        let stsc = mp4_box(b"stsc", &full_box(0, &stsc_body));

        let mut stco_body = 1u32.to_be_bytes().to_vec();
        stco_body.extend_from_slice(&100u32.to_be_bytes());
        let stco = mp4_box(b"stco", &full_box(0, &stco_body));

        let url = mp4_box(b"url ", &[0, 0, 0, 1]);
        let mut dref_body = 1u32.to_be_bytes().to_vec();
        dref_body.extend_from_slice(&url);
        let dref = mp4_box(b"dref", &full_box(0, &dref_body));
        let dinf = mp4_box(b"dinf", &dref);

        let stbl = mp4_box(
            b"stbl",
            &concat_boxes(&[stsd, stts, stss, stsz, stsc, stco]),
        );
        let minf = mp4_box(b"minf", &concat_boxes(&[dinf, stbl]));
        let mdia = mp4_box(b"mdia", &concat_boxes(&[mdhd, hdlr, minf]));
        let trak = mp4_box(b"trak", &mdia);
        mp4_box(b"moov", &trak)
    }

    #[test]
    fn parses_video_timeline_and_byte_ranges() {
        let track = parse_mp4_video_track(&synthetic_mp4(*b"avc1")).unwrap();
        assert_eq!(track.timescale, 1_000);
        assert_eq!(track.presentation_start, 0);
        assert_eq!(track.presentation_end, 3_000);
        assert_eq!(track.samples.len(), 3);
        assert_eq!(track.samples[0].byte_offset, 100);
        assert_eq!(track.samples[1].byte_offset, 110);
        assert_eq!(track.samples[2].byte_offset, 130);
        assert!(track.samples[0].is_sync);
        assert!(!track.samples[1].is_sync);
        assert!(track.samples[2].is_sync);
    }

    #[test]
    fn accepts_legacy_signed_offsets_in_version_zero_ctts() {
        let mut body = 2u32.to_be_bytes().to_vec();
        body.extend_from_slice(&1u32.to_be_bytes());
        body.extend_from_slice(&0u32.to_be_bytes());
        body.extend_from_slice(&1u32.to_be_bytes());
        body.extend_from_slice(&(-2_000i32 as u32).to_be_bytes());
        assert_eq!(
            parse_ctts(Some(&full_box(0, &body)), 2).unwrap(),
            vec![0, -2_000]
        );
    }

    #[test]
    fn rejects_non_h264_video() {
        assert_eq!(
            parse_mp4_video_track(&synthetic_mp4(*b"vp09")),
            Err(Mp4Error::UnsupportedCodec(*b"vp09"))
        );
    }

    #[test]
    fn rejects_fragmented_mp4() {
        assert_eq!(
            parse_mp4_video_track(&mp4_box(b"moof", &[])),
            Err(Mp4Error::FragmentedMp4)
        );
    }

    #[test]
    fn rejects_truncated_box() {
        assert_eq!(
            parse_mp4_video_track(&[0, 0, 0, 12, b'm', b'o', b'o', b'v']),
            Err(Mp4Error::InvalidBoxSize)
        );
    }
}
