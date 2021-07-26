const HtmlWebpackPlugin = require('html-webpack-plugin');
const WasmPackPlugin = require("@wasm-tool/wasm-pack-plugin");
const { WebpackManifestPlugin } = require('webpack-manifest-plugin');
const WorkboxPlugin = require('workbox-webpack-plugin');
const FaviconsWebpackPlugin = require('favicons-webpack-plugin');
const CopyPlugin = require("copy-webpack-plugin");
const DotEnv = require('dotenv-webpack');
const path = require('path');

const plugins = [
    new DotEnv({
        systemvars: true
    }),
    new HtmlWebpackPlugin({
        title: 'ThunderForge VTT',
        base: "/",
        hash: true,
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
        crateDirectory: path.resolve(__dirname, "./src/core"),
    }),
    new CopyPlugin({
        patterns: [
            { from: "assets", to: "../assets" }
        ],
    }),
    new FaviconsWebpackPlugin(path.resolve(__dirname, 'assets/favicon.png'))

];

// if (process.env.NODE_ENV === 'production') {
    plugins.push(new WorkboxPlugin.GenerateSW({
        // these options encourage the ServiceWorkers to get in there fast
        // and not allow any straggling "old" SWs to hang around
        clientsClaim: true,
        skipWaiting: true,
    }))
// }

module.exports = {
    entry: './src/client/src/loader.js',
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
    plugins,
    module: {
        rules: [
            {
                test: /\.(sass|css|scss)$/,
                use: [
                    'style-loader',
                    'css-loader',
                    {
                        loader: 'postcss-loader',
                        options: {
                            postcssOptions: {
                                plugins: [require('autoprefixer')()],
                            },
                        },
                    },
                    'sass-loader',
                ],
            },
            {
                test: /\.tsx?$/,
                use: 'ts-loader',
                exclude: /node_modules/,
            },
        ]
    },
    resolve: {
        extensions: ['.tsx', '.ts', '.js'],
    },
    optimization: {
        splitChunks: {
            // include all types of chunks
            chunks: 'all',
        }
    }
};
