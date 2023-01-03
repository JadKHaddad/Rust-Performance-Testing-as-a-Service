module.exports = {
  //publicPath: process.env.NODE_ENV === 'production' ? 'static/' : '/',
  devServer: {
    clientLogLevel: 'info',
    proxy: {
      '/api': {
        target: 'http://localhost:3000/',
        changeOrigin: true,
        ws: true,
        pathRewrite: {
          '^/api': ''
        }
      },
      '/explore': {
        target: 'http://localhost:3000/explore',
        pathRewrite: {
          '^/explore': ''
        }
      }
    }
  }
};