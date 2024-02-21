const path = require("path");

module.exports = {
  entry: {
    "wasm32-wasi": "./src/wasm32-wasi.ts",
    "wasm32-unknown-unknown": "./src/wasm32-unknown-unknown.ts",
  },
  mode: "production",
  module: {
    rules: [
      {
        test: /\.tsx?$/,
        use: "ts-loader",
        exclude: /node_modules/,
      },
    ],
  },
  resolve: {
    extensions: [".tsx", ".ts", ".js"],
  },
  output: {
    filename: "[name].js",
    path: path.resolve(__dirname, "dist"),
  },
};
