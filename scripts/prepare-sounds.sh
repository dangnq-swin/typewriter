#!/usr/bin/env bash
# Cuts the bundled sound clips in assets/sounds/ from their CC0 sources
# (listed in assets/LICENSES.md). Needs curl and ffmpeg. The downloads are
# cached in target/sound-sources/ and are not part of the repository.
set -euo pipefail

root=$(cd "$(dirname "$0")/.." && pwd)
cache="$root/target/sound-sources"
out="$root/assets/sounds"
mkdir -p "$cache" "$out"

bsb=https://bigsoundbank.com/UPLOAD/flac
fs=https://cdn.freesound.org/previews
declare -A sources=(
    [hermes-key.flac]=$bsb/2842.flac
    [hermes-typing.flac]=$bsb/2839.flac
    [hermes-space.flac]=$bsb/2843.flac
    [hermes-bell-1.flac]=$bsb/2844.flac
    [hermes-bell-2.flac]=$bsb/2845.flac
    # Only offered as a 320 kbps MP3 (an older recording).
    [newspaper-ball.mp3]=https://bigsoundbank.com/UPLOAD/mp3/0670.mp3
    [carriage-return.mp3]=$fs/318/318686_1147663-hq.mp3
    [gate13.mp3]=$fs/697/697389_5135931-hq.mp3
    [platen-ratchet.mp3]=$fs/761/761339_10683427-hq.mp3
    [eraser.mp3]=$fs/154/154461_2592491-hq.mp3
    [fluid-brush.mp3]=$fs/482/482891_4023776-hq.mp3
)
for name in "${!sources[@]}"; do
    # Renamed only when complete, so an interrupted download is not reused.
    [[ -s $cache/$name ]] || {
        curl -sfL -o "$cache/$name.part" "${sources[$name]}"
        mv "$cache/$name.part" "$cache/$name"
    }
done

# clip <output> <source> <start s> <end s> <peak dBFS> [extra ffmpeg filters]
clip() {
    local name=$1 src=$cache/$2 start=$3 end=$4 peak=$5 extra=${6:-}
    local fade tmp max gain
    # Fade the tail over a quarter of the clip, at most 0.1 s.
    fade=$(awk -v s="$start" -v e="$end" 'BEGIN { f = (e - s) / 4; print (f > 0.1 ? 0.1 : f) }')
    local filters="atrim=$start:$end,asetpts=PTS-STARTPTS,aformat=channel_layouts=mono,aresample=48000"
    [[ -n $extra ]] && filters+=",$extra"
    filters+=",afade=t=in:d=0.003,areverse,afade=t=in:d=$fade,areverse"
    tmp=$(mktemp --suffix=.wav)
    ffmpeg -v error -y -i "$src" -af "$filters" -c:a pcm_f32le "$tmp"
    normalize "$tmp" "$name" "$peak"
}

# normalize <temporary wav> <output> <peak dBFS>: writes the output at that
# peak level and removes the temporary file.
normalize() {
    local tmp=$1 name=$2 peak=$3 max gain
    max=$(ffmpeg -i "$tmp" -af volumedetect -f null - 2>&1 | sed -n 's/.*max_volume: \(-\?[0-9.]*\) dB/\1/p')
    gain=$(awk -v p="$peak" -v m="$max" 'BEGIN { print p - m }')
    ffmpeg -v error -y -i "$tmp" -af "volume=${gain}dB" -c:a pcm_s16le "$out/$name"
    rm -f "$tmp"
}

# duration <wav>: length in seconds.
duration() {
    ffprobe -v error -show_entries format=duration -of csv=p=0 "$1"
}

# Key strikes start just before the typebar hits, so there is no lag after
# the key press. Each keeps the typebar's return click.
clip key-1.wav hermes-key.flac 0.150 0.430 -6
clip key-2.wav hermes-typing.flac 0.380 0.660 -6
clip key-3.wav hermes-typing.flac 1.425 1.705 -6
clip key-4.wav hermes-typing.flac 3.015 3.295 -6
clip key-5.wav hermes-typing.flac 3.355 3.635 -6
clip key-6.wav hermes-typing.flac 5.395 5.675 -6
clip space.wav hermes-space.flac 0.090 0.750 -10
# The escapement click after a strike, without the strike itself.
clip backspace.wav hermes-key.flac 0.265 0.550 -16
# A muffled strike: the key goes down but the carriage will not move.
clip blocked.wav hermes-key.flac 0.150 0.290 -14 "lowpass=f=700,lowpass=f=700"
clip bell-1.wav hermes-bell-1.flac 0.040 2.050 -12
clip bell-2.wav hermes-bell-2.flac 0.080 2.300 -12
clip return.wav carriage-return.mp3 0.080 1.140 -10
# The end of the carriage's slide and its stop.
clip tab.wav carriage-return.mp3 0.300 0.950 -12
# Single clicks of the platen ratchet, one per line rolled by hand.
clip roll-1.wav platen-ratchet.mp3 0.485 0.585 -14
clip roll-2.wav platen-ratchet.mp3 0.695 0.795 -14
clip roll-3.wav platen-ratchet.mp3 1.120 1.220 -14
clip roll-4.wav platen-ratchet.mp3 2.430 2.530 -14

# Feeding a sheet is two clips played back to back: the finished sheet
# winding out, then the new one winding in.

# feed-in.wav: winding the new sheet in, from Gate13 1:49-1:57 (picked by ear:
# the take narrates each action). Its three parts (paper going in, the page
# turning over the platen, the ratchet run) are kept, with the pauses between
# them tightened to 0.15 s and 0.25 s. Cuts are in the quiet between parts.
part() { # part <start> <end> <pause after>
    echo "atrim=$1:$2,asetpts=PTS-STARTPTS,afade=t=in:d=0.01,areverse,afade=t=in:d=0.03,areverse,apad=pad_dur=$3"
}
tmp=$(mktemp --suffix=.wav)
ffmpeg -v error -y -i "$cache/gate13.mp3" -filter_complex "
    [0:a]aformat=channel_layouts=mono,aresample=48000,asplit=3[s1][s2][s3];
    [s1]$(part 109.15 110.65 0.15)[a];
    [s2]$(part 110.95 112.75 0.25)[b];
    [s3]$(part 113.85 117.00 0)[c];
    [a][b][c]concat=n=3:v=0:a=1" -c:a pcm_f32le "$tmp"
normalize "$tmp" feed-in.wav -12

# feed-out.wav: the finished sheet wound out, a steady ratchet click every
# CLICK_SECONDS. It lasts as long as the pauses tightened in feed-in.wav gave
# back (8 s less feed-in.wav), stretched by WIND_OUT_STRETCH so the sheet does
# not rush out.
CLICK_SECONDS=0.06
WIND_OUT_STRETCH=1.4
out_seconds=$(awk -v i="$(duration "$out/feed-in.wav")" -v s="$WIND_OUT_STRETCH" 'BEGIN { print (8.0 - i) * s }')
clicks=$(awk -v d="$out_seconds" -v c="$CLICK_SECONDS" 'BEGIN { print int((d - 0.1) / c) + 1 }')
order=(1 3 2 4 2 1 4 3) # the variants in an uneven order, so no pattern is heard
inputs=() graph="" mix=""
for ((i = 0; i < clicks; i++)); do
    inputs+=(-i "$out/roll-${order[i % ${#order[@]}]}.wav")
    delay=$(awk -v i="$i" -v c="$CLICK_SECONDS" 'BEGIN { print int(i * c * 1000) }')
    graph+="[$i:a]adelay=$delay[c$i];"
    mix+="[c$i]"
done
tmp=$(mktemp --suffix=.wav)
ffmpeg -v error -y "${inputs[@]}" -filter_complex \
    "${graph}${mix}amix=inputs=$clicks:normalize=0,apad,atrim=0:$out_seconds" -c:a pcm_f32le "$tmp"
normalize "$tmp" feed-out.wav -14
clip erase.wav eraser.mp3 1.000 1.650 -18
# A dab of correction fluid: the wet brush pressed onto the paper (the take's
# louder moments are the brush knocking, so they are left out).
clip fluid.wav fluid-brush.mp3 9.700 10.200 -18 "highpass=f=150"
# A sheet scrunched up into a ball: one burst of newspaper crumpling.
clip crumple.wav newspaper-ball.mp3 23.250 24.150 -14
