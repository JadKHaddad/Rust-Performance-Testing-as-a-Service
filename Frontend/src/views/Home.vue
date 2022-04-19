<template>
  <div>
    <form>
      <input type="file" webkitdirectory mozdirectory ref="files" />
      <div>Please make sure all names don't include blank spaces</div>
      <button type="button" @click="upload">Upload</button>
      <div>percentCompleted: {{ percentCompleted }}</div>
      <div>uploading: {{ uploading }}</div>
    </form>
  </div>
</template>

<script>
export default {
  name: "Home",
  data() {
    return {
      uploading: false,
      percentCompleted: 0,
    };
  },
  methods: {
    upload() {
      const files = this.$refs.files.files;
      if (files.length < 1) {
        console.log("Please select a directory to upload");
        return false;
      }
      var data = new FormData();
      for (var i = 0; i < files.length; i++) {
        data.append("file" + i, files[i]);
      }
      this.uploading = true;

      axios
        .request({
          method: "post",
          url: "/api/upload",
          data: data,
          onUploadProgress: (progressEvent) => {
            this.percentCompleted = Math.round(
              (progressEvent.loaded * 100) / progressEvent.total
            );
          },
        })
        .then((response) => {
          this.uploading = false;
          const data = response.data;
          console.log(data);
          if (data.success) {
          } else {
          }
        })
        .catch(() => {
          this.uploading = false;
          console.log("Connection error");
        });
      return false;
    },
  },
};
</script>
