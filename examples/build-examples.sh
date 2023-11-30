
projects=(`find  . ! -path ./Cargo.toml -name "Cargo.toml"`)
arraylength=${#projects[@]}
for (( i=0; i<${arraylength}; i++ ));
do
  if [ ${projects[$i]} = "./fibonacci-input/Cargo.toml" ]; then
    cargo build --features=std --manifest-path ${projects[$i]}
  elif [ ${projects[$i]} = "./stdin/Cargo.toml" ]; then
    cargo build --features=std --manifest-path ${projects[$i]}
  else
    cargo build --manifest-path ${projects[$i]}
  fi
done
