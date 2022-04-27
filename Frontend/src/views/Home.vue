<template>
  <div>
    <form>
      <input type="file" webkitdirectory mozdirectory ref="files" />
      <div>Please make sure all names don't include blank spaces</div>
      <button type="button" @click="upload">Upload</button>
    </form>
    <div>uploading: {{ uploading }}</div>
    <div>response: {{ uploadResponse }}</div>
    <h1>Projects</h1>

    <ul>
      <li v-for="project in projects" :key="project.id">
        {{ project.id }}
        <ul>
          <li v-for="script in project.scripts" :key="script">
            <router-link
              :to="{ name: 'Script', params: { pid: project.id, id: script } }"
            >
              {{ script }}
            </router-link>
          </li>
        </ul>
      </li>
    </ul>
  </div>
</template>

<script>
export default {
  name: "Home",
  data() {
    return {
      uploading: false,
      uploadResponse: "",
      projects: [],
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

      // axios
      //   .request({
      //     method: "post",
      //     url: "/api/upload",
      //     data: data,
      //     onUploadProgress: (progressEvent) => {
      //       this.percentCompleted = Math.round(
      //         (progressEvent.loaded * 100) / progressEvent.total
      //       );
      //     },
      //   })
      //   .then((response) => {
      //     this.uploading = false;
      //     const data = response.data;
      //     this.uploadResponse = data;
      //     console.log(data);
      //     if (data.success) {
      //     } else {
      //     }
      //   })
      //   .catch(() => {
      //     this.uploading = false;
      //     console.log("Connection error");
      //   });

      fetch("/api/master/upload", {
        method: "POST",
        body: data,
      })
        .then((data) => data.json())
        .then((data) => {
          this.uploadResponse = data;
          this.uploading = false;
          console.log(data);
        })
        .catch(() => {
          this.uploading = false;
        });
      return false;
    },
  },
  created() {
    fetch("/api/master/projects")
      .then((data) => data.json())
      .then((data) => {
        this.projects = data.content.projects;
        console.log(data);
      })
      .catch();
  },
};
</script>
