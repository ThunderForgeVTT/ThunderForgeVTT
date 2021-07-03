const HtmlWebpackPlugin = require('html-webpack-plugin');
const WasmPackPlugin = require("@wasm-tool/wasm-pack-plugin");
const path = require('path');

module.exports = {
    entry: './src/loader.js',
    mode: "development",
    devServer: {
        historyApiFallback: true
    },
    experiments: {
        asyncWebAssembly: true
    },
    output: {
        path: path.resolve(__dirname, './dist'),
        filename: "static/js/[name].js",
        chunkFilename: "static/js/[name].chunk.js",
    },
    plugins: [
        new HtmlWebpackPlugin({
            title: 'ThunderForge VTT'
        }),
        new WasmPackPlugin({
            crateDirectory: path.resolve(__dirname, "./src/client"),
        })
    ],
    optimization: {
        splitChunks: {
            // include all types of chunks
            chunks: 'all',
        }
    }
};
