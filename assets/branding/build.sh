#!/bin/sh
# Render the VorT2x placeholder artwork from the SVGs in this folder.
# Needs rsvg-convert, ImageMagick and the DejaVu fonts, e.g. in Docker:
#   docker run --rm -v "$PWD/assets":/a -w /a/branding debian:trixie sh -c \
#     'apt-get update -qq && apt-get install -y -qq librsvg2-bin imagemagick fonts-dejavu-core >/dev/null && sh build.sh'
set -e
cd "$(dirname "$0")"
A=..
r() { rsvg-convert "$@"; }
fit() { # svg, width, height, out: fit inside WxH, transparent, centred
	r -h "$3" "$1" -o /tmp/fit.png
	convert /tmp/fit.png -background none -gravity center -resize "${2}x${3}" -extent "${2}x${3}" "$4"
}
fit wordmark-dark.svg 160 42 "$A/vort2x-app-logo.png"
fit wordmark-light.svg 160 42 "$A/vort2x-app-logo-light.png"
fit wordmark-dark.svg 120 32 "$A/gdm/vort2x-gdm-logo.png"
r -w 1024 -h 1024 logo-stacked.svg -o "$A/vorT2x-logo-tr.png"
cp logo-stacked.svg "$A/vorT2x-logo-tr.svg"
r -w 900 wordmark-dark.svg -o /tmp/wide.png
convert /tmp/wide.png -background black -gravity center -extent 1536x1024 -flatten "$A/vorT2x-logo.png"
r splash-mark.svg -o "$A/plymouth/vort2x/watermark.png"
cp "$A/plymouth/vort2x/watermark.png" "$A/plymouth/vort2x/animation-0001.png"
ls -l "$A"/vort2x-app-logo*.png "$A"/gdm/vort2x-gdm-logo.png "$A"/vorT2x-logo*.png "$A"/plymouth/vort2x/watermark.png
# Patreon "Support Tier" image (Patreon asks for 460x200; @2x for sharp screens)
r -w 460 -h 200 patreon-support-tier.svg -o patreon-support-tier.png
r -w 920 -h 400 patreon-support-tier.svg -o patreon-support-tier@2x.png
