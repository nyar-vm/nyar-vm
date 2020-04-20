# shellcheck disable=SC2164
# shellcheck disable=SC2103
cd projects || cd ../projects

cd nyar-ast
cargo publish --allow-dirty || cargo bump patch
cd ..

cd valkyrie-parser
cargo publish --allow-dirty || cargo bump patch
cd ..
