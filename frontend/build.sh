#!/bin/sh

cd "$(dirname "$0")"
set -e

export NODE_OPTIONS=--openssl-legacy-provider

echo "Cleaning up old environment."
rm -rf dist static public

echo "Installing environment."
npm install

echo "Bundling JavaScript."
./node_modules/.bin/webpack

cp -R static-src static

echo "${NOTIFY_TRACKER}" > tracker.html

echo "Generating HTML files."
gen_html() {
    cat html-src/_header.html html-src/${1}.html tracker.html html-src/_footer.html > static/${1}.html
}

gen_html index
gen_html channel

echo "Merging with static files."
mkdir -p public/
cp -R dist/* static/* public/

echo "Generating redirects file."
echo "/c/* /channel.html 200" >> public/_redirects
echo "/* ${NOTIFY_API_PROXY}/:splat 200" >> public/_redirects

