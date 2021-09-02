const HtmlWebpackPlugin = require("html-webpack-plugin");
const WasmPackPlugin = require("@wasm-tool/wasm-pack-plugin");
const { WebpackManifestPlugin } = require("webpack-manifest-plugin");
const WorkboxPlugin = require("workbox-webpack-plugin");
const FaviconsWebpackPlugin = require("favicons-webpack-plugin");
const CopyPlugin = require("copy-webpack-plugin");
const DotEnv = require("dotenv-webpack");
const path = require("path");
const CompressionPlugin = require("compression-webpack-plugin");
const zlib = require("zlib");

const plugins = [
  new DotEnv({
    systemvars: true,
  }),
  new HtmlWebpackPlugin({
    title: "ThunderForge VTT",
    base: "/",
    hash: true,
    template: path.resolve(__dirname, './src/public/index.html'),
    meta: {
      charset: "UTF-8",
      viewport: "width=device-width, initial-scale=1, shrink-to-fit=no",
    },
  }),
  new WebpackManifestPlugin({

  }),
  new WasmPackPlugin({
    crateDirectory: path.resolve(__dirname, "./src/client"),
  }),
  new WasmPackPlugin({
    crateDirectory: path.resolve(__dirname, "./src/core"),
  }),
  new CopyPlugin({
    patterns: [{ from: "assets", to: "../assets" }],
  }),
  new FaviconsWebpackPlugin(path.resolve(__dirname, "assets/favicon.png")),
];

if (process.env.NODE_ENV === "production") {
  plugins.push(
    new WorkboxPlugin.GenerateSW({
      // these options encourage the ServiceWorkers to get in there fast
      // and not allow any straggling "old" SWs to hang around
      clientsClaim: true,
      skipWaiting: true,
      cleanupOutdatedCaches: true,
      maximumFileSizeToCacheInBytes: 5 * 1024 * 1024,
    })
  );

}
plugins.push(
    new CompressionPlugin({
      filename: "[path][base].br",
      algorithm: "brotliCompress",
      test: /\.(js|css|html|svg|wasm|png|jpeg|gif)$/,
      compressionOptions: {
        params: {
          [zlib.constants.BROTLI_PARAM_QUALITY]: 11,
        },
      },
      threshold: 10240,
      minRatio: 0.8,
      deleteOriginalAssets: false,
    })
);

module.exports = {
  entry: "./src/client/src/loader.js",
  mode: "development",
  devServer: {
    historyApiFallback: true,
  },
  experiments: {
    asyncWebAssembly: true,
  },
  output: {
    path: path.resolve(__dirname, "./data/client"),
    filename: "static/js/[name].js",
    chunkFilename: "static/js/[name].chunk.js",
  },
  plugins,
  module: {
    rules: [
      {
        test: /\.(sass|css|scss)$/,
        use: [
          "thread-loader",
          "style-loader",
          "css-loader",
          {
            loader: "postcss-loader",
            options: {
              postcssOptions: {
                plugins: [require("autoprefixer")()],
              },
            },
          },
          "sass-loader",
        ],
      },
      {
        test: /\.m?[tj]sx?$/,
        use: [
          "thread-loader",
          {
            loader: "swc-loader",
          },
        ],
        exclude: /node_modules/,
        enforce: "pre",
      },
    ],
  },
  resolve: {
    extensions: [".tsx", ".ts", ".js", ".css", ".scss", "..."],
  },
  optimization: {
    runtimeChunk: true,
    removeEmptyChunks: true,
    splitChunks: {
      chunks: "async",
      minSize: 20000,
      minRemainingSize: 0,
      minChunks: 1,
      maxAsyncRequests: 30,
      maxInitialRequests: 30,
      enforceSizeThreshold: 50000,
      cacheGroups: {
        defaultVendors: {
          test: /[\\/]node_modules[\\/]/,
          name: "vendors",
          priority: -10,
          reuseExistingChunk: true,
        },
        default: {
          minChunks: 2,
          priority: -20,
          reuseExistingChunk: true,
        },
      },
    },
  },
};
