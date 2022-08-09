#!/bin/bash

set -e
set -x

rm -fr ./dist/
rm -fr _intermediate/

mkdir -p _intermediate/mjs
cp -r src/** _intermediate/mjs/
pnpm dlx renamer --find .ts --replace .mts "_intermediate/mjs/**/*"
xs_cmd node:replace_import_ext --userPath _intermediate/mjs --inputFileExt mts --inputImportExt js --importExt mjs

mkdir -p _intermediate/cjs
cp -r src/** _intermediate/cjs/
pnpm dlx renamer --find .ts --replace .cts "_intermediate/cjs/**/*"
xs_cmd node:replace_import_ext --userPath _intermediate/cjs --inputFileExt cts --inputImportExt js --importExt cjs

tsc -p tsconfig.mjs.json
tsc -p tsconfig.cjs.json

rm -fr _intermediate