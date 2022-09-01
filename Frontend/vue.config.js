module.exports = {
  //publicPath: process.env.NODE_ENV === 'production' ? 'static/' : '/',
  devServer: {
    clientLogLevel: 'info',
    proxy: {
      '/api/master': {
        target: 'http://localhost:3000/',
        changeOrigin: true,
        ws: true,
        pathRewrite: {
          '^/api/master': ''
        }
      },
      '/api/worker': {
        target: 'http://localhost:6000/',
        changeOrigin: true,
        ws: true,
        pathRewrite: {
          '^/api/worker': ''
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