#!/bin/bash
# OASIS Mesh demo — montage Vidéo B (60-90s) à partir de demo-mesh-3plat.mp4
# Structure:
#   0-5s    : title card
#   5-40s   : source split-screen + overlays rectangles rouges sur événements
#   40-70s  : zoom MESH-LIVE terminal (bottom-right region 480,540 1440x540)
#   70-85s  : recap card statique
#   85-90s  : end card

FFMPEG="C:/Users/pc/AppData/Local/Microsoft/WinGet/Packages/Gyan.FFmpeg_Microsoft.Winget.Source_8wekyb3d8bbwe/ffmpeg-8.1-full_build/bin/ffmpeg.exe"
SRC="c:/dev/oasis/mesh/demo-mesh-3plat.mp4"
OUT="c:/dev/oasis/mesh/demo-mesh-3plat-montage.mp4"
FONT="C\\:/Windows/Fonts/arial.ttf"
FONTBOLD="C\\:/Windows/Fonts/arialbd.ttf"

rm -f "$OUT"

# Take a window inside source where mesh is densest (after warmup baked in already, T=60-150s of source)
# - Split-screen segment: source 60-95s mapped to montage 5-40s (35s)
# - Zoom segment: source 95-125s mapped to montage 40-70s (30s) + crop bottom-right
# - Static recap: 15s solid color with metrics

"$FFMPEG" -y \
  -f lavfi -i "color=c=black:s=1920x1080:d=5" \
  -ss 60 -t 35 -i "$SRC" \
  -ss 95 -t 30 -i "$SRC" \
  -f lavfi -i "color=c=black:s=1920x1080:d=15" \
  -f lavfi -i "color=c=black:s=1920x1080:d=5" \
  -filter_complex "
    [0:v]drawtext=fontfile='$FONTBOLD':text='OASIS':fontsize=110:fontcolor=white:x=(w-text_w)/2:y=320,
         drawtext=fontfile='$FONT':text='cross-platform federation demo':fontsize=52:fontcolor=#cccccc:x=(w-text_w)/2:y=470,
         drawtext=fontfile='$FONT':text='1 phone  +  3 sim drones  +  1 synthetic node':fontsize=40:fontcolor=#aaaaaa:x=(w-text_w)/2:y=580,
         drawtext=fontfile='$FONT':text='T+120s observation window':fontsize=34:fontcolor=#888888:x=(w-text_w)/2:y=680,
         fade=in:0:8,fade=out:st=4:d=1[intro];

    [1:v]drawbox=x=10:y=180:w=460:h=50:color=red@0.85:t=5:enable='between(t,2,7)+between(t,15,20)+between(t,28,33)',
         drawtext=fontfile='$FONTBOLD':text='SPORE received':fontsize=26:fontcolor=red:box=1:boxcolor=black@0.7:boxborderw=8:x=15:y=140:enable='between(t,2,7)+between(t,15,20)+between(t,28,33)',
         drawbox=x=510:y=560:w=1390:h=40:color=red@0.85:t=5:enable='between(t,8,13)+between(t,21,26)+between(t,34,35)',
         drawtext=fontfile='$FONTBOLD':text='MESH merged from phone':fontsize=26:fontcolor=red:box=1:boxcolor=black@0.7:boxborderw=8:x=515:y=515:enable='between(t,8,13)+between(t,21,26)+between(t,34,35)',
         drawtext=fontfile='$FONT':text='Phone (Termux)  |  Webots 3 drones  |  Mesh log':fontsize=28:fontcolor=white:box=1:boxcolor=black@0.6:boxborderw=10:x=(w-text_w)/2:y=20[seg1];

    [2:v]crop=1440:540:480:540,scale=1920:1080,
         drawtext=fontfile='$FONTBOLD':text='MESH-LIVE — counters climbing':fontsize=44:fontcolor=#00ff88:box=1:boxcolor=black@0.7:boxborderw=14:x=(w-text_w)/2:y=20[seg2];

    [3:v]drawtext=fontfile='$FONTBOLD':text='cross-platform merge events':fontsize=64:fontcolor=#00ff88:x=(w-text_w)/2:y=140,
         drawtext=fontfile='$FONTBOLD':text='Phone -> Webots\\:  78 merges (35 patrol2, 39 supervisor, 4 patrol1)':fontsize=38:fontcolor=white:x=(w-text_w)/2:y=320,
         drawtext=fontfile='$FONTBOLD':text='Phone <- mesh   \\:  401 SPORE received (foreign digests)':fontsize=38:fontcolor=white:x=(w-text_w)/2:y=400,
         drawtext=fontfile='$FONT':text='format binaire OASISMEM   |   ADB pull/push   |   no broker':fontsize=32:fontcolor=#aaaaaa:x=(w-text_w)/2:y=520,
         drawtext=fontfile='$FONT':text='5 OASIS kernels active simultaneously on 3 hardware platforms':fontsize=30:fontcolor=#888888:x=(w-text_w)/2:y=620,
         drawtext=fontfile='$FONT':text='(observation: 5 min recording, 60 s warmup before t=0)':fontsize=24:fontcolor=#666666:x=(w-text_w)/2:y=720[seg3];

    [4:v]drawtext=fontfile='$FONTBOLD':text='304 cross-platform merges in 120s':fontsize=56:fontcolor=white:x=(w-text_w)/2:y=300,
         drawtext=fontfile='$FONTBOLD':text='No cloud.  No broker.':fontsize=64:fontcolor=#00ff88:x=(w-text_w)/2:y=440,
         drawtext=fontfile='$FONT':text='Full detail\\: DEMO-MESH-3PLATFORMS.md':fontsize=34:fontcolor=#aaaaaa:x=(w-text_w)/2:y=620,
         drawtext=fontfile='$FONT':text='souhaybrharrab@gmail.com':fontsize=34:fontcolor=#aaaaaa:x=(w-text_w)/2:y=700,
         fade=in:0:8[outro];

    [intro][seg1][seg2][seg3][outro]concat=n=5:v=1:a=0[final]
  " \
  -map '[final]' \
  -c:v libx264 -preset medium -crf 21 -pix_fmt yuv420p \
  -movflags +faststart \
  "$OUT" 2>&1 | tail -10

echo
echo "=== Output ==="
ls -la "$OUT"
"$FFMPEG" -i "$OUT" 2>&1 | grep -E "Duration|Stream" | head -3
