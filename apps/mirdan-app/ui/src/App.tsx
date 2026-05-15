import { TooltipProvider } from "@/components/ui/tooltip";
import { ToastContainer } from "@/components/toast";
import { PackageList } from "@/components/package-list";

function App() {
  return (
    <TooltipProvider>
      <div className="h-screen bg-background text-foreground flex flex-col">
        <PackageList />
        <ToastContainer />
      </div>
    </TooltipProvider>
  );
}

export default App;
