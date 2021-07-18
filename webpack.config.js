const HtmlWebpackPlugin = require('html-webpack-plugin');
const WasmPackPlugin = require("@wasm-tool/wasm-pack-plugin");
const { WebpackManifestPlugin } = require('webpack-manifest-plugin');
const WorkboxPlugin = require('workbox-webpack-plugin');
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
        path: path.resolve(__dirname, './data/client'),
        filename: "static/js/[name].js",
        chunkFilename: "static/js/[name].chunk.js",
    },
    plugins: [
        new HtmlWebpackPlugin({
            title: 'ThunderForge VTT',
            meta: {
                charset: "UTF-8",
                viewport: 'width=device-width, initial-scale=1, shrink-to-fit=no'
            }
        }),
        new WebpackManifestPlugin(),
        new WasmPackPlugin({
            crateDirectory: path.resolve(__dirname, "./src/client"),
        }),
        new WasmPackPlugin({
            crateDirectory: path.resolve(__dirname, "./src/engine"),
        }),
        new WasmPackPlugin({
            crateDirectory: path.resolve(__dirname, "./src/core"),
        }),
        new WorkboxPlugin.GenerateSW({
            // these options encourage the ServiceWorkers to get in there fast
            // and not allow any straggling "old" SWs to hang around
            clientsClaim: true,
            skipWaiting: true,
        }),
    ],
    module: {
        rules: [
            {
                test: /\.s[ac]ss$/i,
                use: [
                    // Creates `style` nodes from JS strings
                    "style-loader",
                    // Translates CSS into CommonJS
                    "css-loader",
                    // Compiles Sass to CSS
                    "sass-loader",
                ],
            },
        ]
    },
    optimization: {
        splitChunks: {
            // include all types of chunks
            chunks: 'all',
        }
    }
};
