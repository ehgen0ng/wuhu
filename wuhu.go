package main

import (
	"bufio"
	"fmt"
	"io"
	"io/fs"
	"net/http"
	"os"
	"os/exec"
	"path/filepath"
	"strconv"
	"strings"
	"time"
)

func main() {
	for {
		showMenu()
		choice := getUserChoice()

		switch choice {
		case "1":
			showSteamDirectory()
			waitForExit()
		case "2":
			addAppID()
			waitForEnter()
		case "3":
			organizeAppIDs()
			waitForEnter()
		case "4":
			showAppIDs()
			waitForEnter()
		case "5":
			deleteAppID()
			waitForEnter()
		case "6":
			clearSteamCache()
			waitForEnter()
		default:
			fmt.Println("❌ 输入有误，请重新选择哦~")
			waitForEnter()
		}

		fmt.Println()
	}
}

func showMenu() {
	fmt.Println("")
	fmt.Println(" __      __  __  __  __  __  __  __     ")
	fmt.Println("/\\ \\  __/\\ \\/\\ \\/\\ \\/\\ \\/\\ \\/\\ \\/\\ \\    ")
	fmt.Println("\\ \\ \\/\\ \\ \\ \\ \\ \\ \\ \\ \\ \\_\\ \\ \\ \\ \\ \\   ")
	fmt.Println(" \\ \\ \\ \\ \\ \\ \\ \\ \\ \\ \\ \\  _  \\ \\ \\ \\ \\  ")
	fmt.Println("  \\ \\ \\_/ \\_\\ \\ \\ \\_\\ \\ \\ \\ \\ \\ \\ \\_\\ \\ ")
	fmt.Println("   \\ `\\___x___/\\ \\_____\\ \\_\\ \\_\\ \\_____\\")
	fmt.Println("    '\\/__//__/  \\/_____/\\/_/\\/_/\\/_____/")
	fmt.Println("")
	fmt.Println("            v1.0.1 - Built with Go")
	fmt.Println("")
	fmt.Println("  1. wuhu~")
	fmt.Println("  2. 新增 AppID")
	fmt.Println("  3. 整理 AppID")
	fmt.Println("  4. 查看 AppID")
	fmt.Println("  5. 删除 AppID")
	fmt.Println("  6. 清空 Steam 缓存")
	fmt.Println("")
	fmt.Print("👉 请输入你的选择: ")
}

func getUserChoice() string {
	reader := bufio.NewReader(os.Stdin)
	input, _ := reader.ReadString('\n')
	input = strings.TrimSpace(input)

	if input == "" {
		return "1"
	}

	return input
}

func showAppIDs() {
	fmt.Println("📋 正在扫描 List 目录下的 AppID...")

	err := filepath.WalkDir("List", func(path string, d fs.DirEntry, err error) error {
		if err != nil {
			return err
		}

		if !d.IsDir() && strings.HasSuffix(strings.ToLower(d.Name()), ".txt") && d.Name() != "example.txt" {
			appIDs, err := readAppIDsFromFile(path)
			if err != nil {
				fmt.Printf("  ❌ 无法打开文件 %s: %v\n", path, err)
				return nil
			}

			if len(appIDs) > 0 {
				// 移除.txt扩展名作为文件名显示
				fileName := strings.TrimSuffix(d.Name(), ".txt")
				fmt.Printf("✅ %s.txt\n", fileName)
				for _, appID := range appIDs {
					fmt.Printf("  %s\n", appID)
				}
			}
		}
		return nil
	})

	if err != nil {
		fmt.Printf("❌ 扫描目录失败: %v\n", err)
		return
	}
}

func waitForEnter() {
	fmt.Print("\n按回车键返回主菜单...")
	bufio.NewReader(os.Stdin).ReadLine()
}

func waitForExit() {
	fmt.Print("\n按回车键退出...")
	bufio.NewReader(os.Stdin).ReadLine()
	os.Exit(0)
}

func addAppID() {
	fmt.Print("请输入要添加的 AppID: ")
	reader := bufio.NewReader(os.Stdin)
	input, _ := reader.ReadString('\n')
	appID := strings.TrimSpace(input)

	if appID == "" {
		fmt.Println("❌ AppID 不能为空")
		return
	}

	// 验证是否为有效数字
	if !isValidAppID(appID) {
		fmt.Println("❌ AppID 必须是数字")
		return
	}

	// 确保 List 目录存在
	if err := os.MkdirAll("List", 0755); err != nil {
		fmt.Printf("❌ 创建目录失败: %v\n", err)
		return
	}

	// 检查是否已存在
	if isAppIDExists(appID) {
		fmt.Printf("⚠️  AppID %s 已存在\n", appID)
		return
	}

	// 添加到 go.txt
	goFile := filepath.Join("List", "go.txt")
	file, err := os.OpenFile(goFile, os.O_APPEND|os.O_CREATE|os.O_WRONLY, 0644)
	if err != nil {
		fmt.Printf("❌ 无法打开文件 %s: %v\n", goFile, err)
		return
	}
	defer file.Close()

	if _, err := file.WriteString(appID + "\n"); err != nil {
		fmt.Printf("❌ 写入文件失败: %v\n", err)
		return
	}

	fmt.Printf("✅ 成功添加 AppID %s\n", appID)
}

func deleteAppID() {
	fmt.Print("请输入要删除的 AppID: ")
	reader := bufio.NewReader(os.Stdin)
	input, _ := reader.ReadString('\n')
	appID := strings.TrimSpace(input)

	if appID == "" {
		fmt.Println("❌ AppID 不能为空")
		return
	}

	// 验证是否为有效数字
	if !isValidAppID(appID) {
		fmt.Println("❌ AppID 必须是数字")
		return
	}

	found := false

	err := filepath.WalkDir("List", func(path string, d fs.DirEntry, err error) error {
		if err != nil {
			return err
		}

		if !d.IsDir() && strings.HasSuffix(strings.ToLower(d.Name()), ".txt") {
			if deleteFromFile(path, appID) {
				fmt.Printf("✅ 从 %s 删除了 AppID %s\n", path, appID)
				found = true
			}
		}
		return nil
	})

	if err != nil {
		fmt.Printf("❌ 扫描目录失败: %v\n", err)
		return
	}

	if !found {
		fmt.Printf("❌ 未找到 AppID %s\n", appID)
	}
}

type AppInfo struct {
	AppID  string `json:"appid"`
	Common struct {
		Name          string `json:"name"`
		NameLocalized struct {
			SChinese string `json:"schinese"`
		} `json:"name_localized"`
	} `json:"common"`
}

type SteamAPIResponse struct {
	AppInfo AppInfo `json:"appinfo"`
}

func organizeAppIDs() {
	fmt.Println("📋 开始整理 AppID...")

	allAppIDs := make(map[string]bool)
	err := filepath.WalkDir("List", func(path string, d fs.DirEntry, err error) error {
		if err != nil {
			return err
		}

		if !d.IsDir() && strings.HasSuffix(strings.ToLower(d.Name()), ".txt") && d.Name() != "example.txt" {
			appIDs, err := readAppIDsFromFile(path)
			if err != nil {
				return nil
			}
			for _, appID := range appIDs {
				allAppIDs[appID] = true
			}
		}
		return nil
	})

	if err != nil {
		fmt.Printf("❌ 扫描目录失败: %v\n", err)
		return
	}

	if len(allAppIDs) == 0 {
		fmt.Println("📭 未找到任何 AppID")
		return
	}

	// 创建输出目录在List下
	outputDir := "List"

	// 为每个AppID获取游戏信息并按名称组织
	gameFiles := make(map[string][]string)   // 游戏名称 -> AppID列表
	parentCache := make(map[string]GameInfo) // 缓存parent信息避免重复请求

	for appID := range allAppIDs {
		gameInfo := getGameInfo(appID)
		if gameInfo.Name == "" {
			fmt.Printf("  %s - 获取失败\n", appID)
			continue
		}

		// 处理DLC逻辑
		var targetName string
		if gameInfo.Parent != "" {
			// 这是DLC，检查parent是否在已有AppID中
			if allAppIDs[gameInfo.Parent] {
				// parent在已有AppID中，获取parent信息
				var parentInfo GameInfo
				if cached, exists := parentCache[gameInfo.Parent]; exists {
					parentInfo = cached
				} else {
					parentInfo = getGameInfo(gameInfo.Parent)
					parentCache[gameInfo.Parent] = parentInfo
				}

				if parentInfo.Name != "" {
					targetName = parentInfo.Name
				} else {
					targetName = gameInfo.Name
				}
			} else {
				// parent不在已有AppID中，使用自己的名称
				targetName = gameInfo.Name
			}
		} else {
			targetName = gameInfo.Name
		}

		// 清理文件名，移除非法字符
		safeFileName := sanitizeFileName(targetName)
		gameFiles[safeFileName] = append(gameFiles[safeFileName], appID)
	}

	// 创建文件并删除源AppID
	for fileName, appIDs := range gameFiles {
		filePath := filepath.Join(outputDir, fileName+".txt")
		file, err := os.Create(filePath)
		if err != nil {
			fmt.Printf("❌ 创建文件 %s 失败: %v\n", fileName, err)
			continue
		}

		for _, appID := range appIDs {
			file.WriteString(appID + "\n")
		}
		file.Close()

		fmt.Printf("✅ %s.txt\n", fileName)
		for _, appID := range appIDs {
			fmt.Printf("  %s\n", appID)
		}

		// 删除源AppID（除非已经在目标文件中）
		targetFileName := fileName + ".txt"
		err = filepath.WalkDir("List", func(path string, d fs.DirEntry, err error) error {
			if err != nil || d.IsDir() {
				return err
			}

			// 跳过目标文件本身和example.txt
			if d.Name() == targetFileName || d.Name() == "example.txt" {
				return nil
			}

			if strings.HasSuffix(strings.ToLower(d.Name()), ".txt") {
				for _, appID := range appIDs {
					deleteFromFile(path, appID)
				}
			}
			return nil
		})

		if err != nil {
			// fmt.Printf("⚠️ 清理源文件时出错: %v\n", err)
		}
	}
}

type GameInfo struct {
	Name   string
	Parent string
}

// 清理文件名，移除Windows不允许的字符
func sanitizeFileName(name string) string {
	// Windows不允许的字符: < > : " | ? * \ /
	invalidChars := []string{"<", ">", ":", "\"", "|", "?", "*", "\\", "/"}
	result := name
	for _, char := range invalidChars {
		result = strings.ReplaceAll(result, char, "_")
	}
	// 限制文件名长度
	if len(result) > 100 {
		result = result[:100]
	}
	return strings.TrimSpace(result)
}

func getGameInfo(appID string) GameInfo {
	url := fmt.Sprintf("https://steamui.com/api/get_appinfo.php?appid=%s", appID)

	client := &http.Client{
		Timeout: 15 * time.Second,
	}

	resp, err := client.Get(url)
	if err != nil {
		return GameInfo{}
	}
	defer resp.Body.Close()

	if resp.StatusCode != 200 {
		return GameInfo{}
	}

	body, err := io.ReadAll(resp.Body)
	if err != nil {
		return GameInfo{}
	}

	content := string(body)

	var name, parent string

	// 提取中文名称
	if schinese := extractVDFValue(content, "schinese"); schinese != "" {
		name = schinese
	} else if gameName := extractVDFValue(content, "name"); gameName != "" {
		name = gameName
	}

	// 提取parent信息
	parent = extractVDFValue(content, "parent")

	return GameInfo{
		Name:   name,
		Parent: parent,
	}
}

func extractVDFValue(content, key string) string {
	lines := strings.Split(content, "\n")
	inCommonSection := false
	inNameLocalizedSection := false

	for _, line := range lines {
		line = strings.TrimSpace(line)

		// 检查是否进入common段
		if strings.Contains(line, "\"common\"") {
			inCommonSection = true
			continue
		}

		// 检查是否进入name_localized段
		if inCommonSection && strings.Contains(line, "\"name_localized\"") {
			inNameLocalizedSection = true
			continue
		}

		// 如果在name_localized段中查找schinese
		if key == "schinese" && inNameLocalizedSection {
			if strings.Contains(line, "\"schinese\"") && strings.Count(line, "\"") >= 4 {
				parts := strings.Split(line, "\"")
				if len(parts) >= 4 {
					return parts[3]
				}
			}
		}

		// 如果在common段中查找name（但不在name_localized段中）
		if key == "name" && inCommonSection && !inNameLocalizedSection {
			if strings.Contains(line, "\"name\"") && strings.Count(line, "\"") >= 4 {
				parts := strings.Split(line, "\"")
				if len(parts) >= 4 && strings.TrimSpace(parts[1]) == "name" {
					return parts[3]
				}
			}
		}

		// 如果在common段中查找parent
		if key == "parent" && inCommonSection && !inNameLocalizedSection {
			if strings.Contains(line, "\"parent\"") && strings.Count(line, "\"") >= 4 {
				parts := strings.Split(line, "\"")
				if len(parts) >= 4 && strings.TrimSpace(parts[1]) == "parent" {
					return parts[3]
				}
			}
		}

		// 检查是否离开某个段落（遇到右大括号）
		if strings.Contains(line, "}") {
			if inNameLocalizedSection {
				inNameLocalizedSection = false
			} else if inCommonSection {
				inCommonSection = false
			}
		}
	}
	return ""
}

func isValidAppID(appID string) bool {
	if appID == "" {
		return false
	}
	_, err := strconv.Atoi(appID)
	return err == nil
}

func readAppIDsFromFile(filePath string) ([]string, error) {
	file, err := os.Open(filePath)
	if err != nil {
		return nil, err
	}
	defer file.Close()

	var appIDs []string
	scanner := bufio.NewScanner(file)
	for scanner.Scan() {
		line := strings.TrimSpace(scanner.Text())
		if isValidAppID(line) {
			appIDs = append(appIDs, line)
		}
	}
	return appIDs, scanner.Err()
}

func isAppIDExists(targetID string) bool {
	exists := false
	filepath.WalkDir("List", func(path string, d fs.DirEntry, err error) error {
		if err != nil || exists {
			return err
		}

		if !d.IsDir() && strings.HasSuffix(strings.ToLower(d.Name()), ".txt") {
			appIDs, err := readAppIDsFromFile(path)
			if err != nil {
				return nil
			}
			for _, appID := range appIDs {
				if appID == targetID {
					exists = true
					return nil
				}
			}
		}
		return nil
	})
	return exists
}

func deleteFromFile(filePath, targetID string) bool {
	file, err := os.Open(filePath)
	if err != nil {
		return false
	}
	defer file.Close()

	var lines []string
	found := false

	scanner := bufio.NewScanner(file)
	for scanner.Scan() {
		line := strings.TrimSpace(scanner.Text())
		if line == targetID {
			found = true
			continue // 跳过要删除的行
		}
		if line != "" {
			lines = append(lines, line)
		}
	}

	if !found {
		return false
	}

	// 如果删除后文件为空，删除整个文件
	if len(lines) == 0 {
		os.Remove(filePath)
		return true
	}

	// 重写文件
	file, err = os.Create(filePath)
	if err != nil {
		return false
	}
	defer file.Close()

	for _, line := range lines {
		file.WriteString(line + "\n")
	}

	return true
}

func showSteamDirectory() {
	// 尝试从注册表获取 Steam 路径
	steamPath := getSteamPathFromRegistry()

	if steamPath != "" {
		fmt.Printf("✅ Steam 安装目录: %s\n", steamPath)

		// 更新 DLLInjector.ini 配置
		if updateDLLInjectorConfig(steamPath) {
			fmt.Println("✅ 已更新 GreenLuma 配置")

			// 执行完整的wuhu流程
			runWuhuProcess()
		}
	} else {
		fmt.Println("❌ 未找到 Steam 安装路径")
	}
}

func getSteamPathFromRegistry() string {
	// 尝试从用户注册表获取
	if path := queryRegistry("HKCU", "SOFTWARE\\Valve\\Steam", "SteamPath"); path != "" {
		return path
	}

	// 尝试从系统注册表获取
	if path := queryRegistry("HKLM", "SOFTWARE\\WOW6432Node\\Valve\\Steam", "InstallPath"); path != "" {
		return path
	}

	return ""
}

func queryRegistry(hive, key, value string) string {
	cmd := exec.Command("reg", "query", hive+"\\"+key, "/v", value)
	output, err := cmd.Output()
	if err != nil {
		return ""
	}

	lines := strings.Split(string(output), "\n")
	for _, line := range lines {
		if strings.Contains(line, value) {
			parts := strings.Fields(line)
			if len(parts) >= 3 {
				// 获取路径部分（可能包含空格）
				pathStart := strings.Index(line, "REG_SZ") + 6
				if pathStart > 6 && pathStart < len(line) {
					path := strings.TrimSpace(line[pathStart:])
					return strings.ReplaceAll(path, "/", "\\")
				}
			}
		}
	}

	return ""
}

func clearSteamCache() {
	fmt.Println("🧹 正在清空 Steam 缓存...")

	exePath := filepath.Join("utils", "GreenLuma", "DeleteSteamAppCache.exe")

	// 检查文件是否存在
	if _, err := os.Stat(exePath); err != nil {
		fmt.Printf("❌ 找不到 %s\n", exePath)
		return
	}

	// 执行清空程序
	cmd := exec.Command(exePath)
	err := cmd.Run()

	if err != nil {
		fmt.Printf("❌ 清空失败: %v\n", err)
	} else {
		fmt.Println("✅ Steam 缓存清空完成")
	}
}

func updateDLLInjectorConfig(steamPath string) bool {
	iniPath := filepath.Join("utils", "GreenLuma", "DLLInjector.ini")

	// 检查ini文件是否存在
	if _, err := os.Stat(iniPath); err != nil {
		fmt.Printf("❌ 配置文件不存在: %s\n", iniPath)
		return false
	}

	// 读取ini文件
	file, err := os.Open(iniPath)
	if err != nil {
		fmt.Printf("❌ 无法读取配置文件: %v\n", err)
		return false
	}
	defer file.Close()

	var lines []string
	scanner := bufio.NewScanner(file)

	for scanner.Scan() {
		line := scanner.Text()

		// 修改配置项
		if strings.HasPrefix(strings.TrimSpace(line), "Exe") && strings.Contains(line, "steam.exe") {
			// 将相对路径改为绝对路径
			steamExePath := filepath.Join(steamPath, "steam.exe")
			lines = append(lines, "Exe = "+steamExePath)
		} else {
			lines = append(lines, line)
		}
	}

	// 写回文件
	file, err = os.Create(iniPath)
	if err != nil {
		fmt.Printf("❌ 无法写入配置文件: %v\n", err)
		return false
	}
	defer file.Close()

	for _, line := range lines {
		file.WriteString(line + "\n")
	}

	return true
}

func runWuhuProcess() {
	// 1. 终止 Steam 进程
	fmt.Println("⏹️ 正在终止 Steam 进程...")
	cmd := exec.Command("taskkill", "/F", "/IM", "steam.exe")
	cmd.Run() // 忽略错误，Steam 可能本来就没运行

	// 2. 生成 AppList
	fmt.Println("📝 正在生成 AppList...")
	if !generateAppList() {
		fmt.Println("❌ 生成 AppList 失败")
		return
	}

	// 3. 执行 DLL 注入器
	fmt.Println("💉 正在启动 GreenLuma...")
	injectorPath := filepath.Join("utils", "GreenLuma", "DLLInjector.exe")

	// 检查文件是否存在
	if _, err := os.Stat(injectorPath); err != nil {
		fmt.Printf("❌ 找不到 DLLInjector.exe: %s\n", injectorPath)
		return
	}

	// 获取绝对路径
	absPath, err := filepath.Abs(injectorPath)
	if err != nil {
		fmt.Printf("❌ 获取绝对路径失败: %v\n", err)
		return
	}

	// 获取工作目录
	workDir := filepath.Dir(absPath)

	// 执行注入器
	cmd = exec.Command(absPath)
	cmd.Dir = workDir

	err = cmd.Start()
	if err != nil {
		fmt.Printf("❌ 启动 GreenLuma 失败: %v\n", err)
	} else {
		fmt.Println("✅ GreenLuma 已启动，Steam 正在加载...")
	}
}

func generateAppList() bool {
	appListDir := filepath.Join("utils", "GreenLuma", "AppList")

	// 创建或清空 AppList 目录
	if _, err := os.Stat(appListDir); err == nil {
		// 目录存在，清空
		if err := os.RemoveAll(appListDir); err != nil {
			fmt.Printf("❌ 清空 AppList 目录失败: %v\n", err)
			return false
		}
	}

	// 创建目录
	if err := os.MkdirAll(appListDir, 0755); err != nil {
		fmt.Printf("❌ 创建 AppList 目录失败: %v\n", err)
		return false
	}

	addedIDs := make(map[string]bool)
	fileIndex := 0

	// 读取 List 目录下所有 txt 文件
	err := filepath.WalkDir("List", func(path string, d fs.DirEntry, err error) error {
		if err != nil {
			return err
		}

		if !d.IsDir() && strings.HasSuffix(strings.ToLower(d.Name()), ".txt") && d.Name() != "example.txt" {
			appIDs, err := readAppIDsFromFile(path)
			if err != nil {
				return nil
			}

			if len(appIDs) > 0 {
				// 移除.txt扩展名作为文件名显示
				fileName := strings.TrimSuffix(d.Name(), ".txt")
				fmt.Printf("✅ %s.txt\n", fileName)
			}

			for _, appID := range appIDs {
				if addedIDs[appID] {
					continue
				}

				// 创建 AppList 文件
				appListFile := filepath.Join(appListDir, fmt.Sprintf("%d.txt", fileIndex))
				if err := writeAppIDToFile(appListFile, appID); err != nil {
					fmt.Printf("❌ 写入 AppList 文件失败: %v\n", err)
					continue
				}

				fmt.Printf("  %s\n", appID)
				addedIDs[appID] = true
				fileIndex++

				// 检查 ManifestHub 中的关联文件
				manifestPath := filepath.Join("utils", "ManifestHub", appID, appID+".txt")
				if relatedIDs, err := readAppIDsFromFile(manifestPath); err == nil {
					for _, relatedID := range relatedIDs {
						if !addedIDs[relatedID] {
							relatedAppListFile := filepath.Join(appListDir, fmt.Sprintf("%d.txt", fileIndex))
							if err := writeAppIDToFile(relatedAppListFile, relatedID); err == nil {
								fmt.Printf("  %s\n", relatedID)
								addedIDs[relatedID] = true
								fileIndex++
							}
						}
					}
				}
			}
		}
		return nil
	})

	if err != nil {
		fmt.Printf("❌ 扫描 List 目录失败: %v\n", err)
		return false
	}

	return true
}

func writeAppIDToFile(filePath, appID string) error {
	file, err := os.Create(filePath)
	if err != nil {
		return err
	}
	defer file.Close()

	_, err = file.WriteString(appID + "\n")
	return err
}
