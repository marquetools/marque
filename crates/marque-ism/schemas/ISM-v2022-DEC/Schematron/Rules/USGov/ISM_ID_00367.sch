<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="BANNER STRUCTURECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00367">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00367][Error] If ISM_USGOV_RESOURCE and attribute @ism:derivedFrom is 
        specified, then attribute @ism:classifiedBy must not be specified.
        
        Human Readable: USA documents that specify a derivative classifier must not also 
        include information related to Original Classification Authorities (classificationReason and classifiedBy).
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
    	If the document is an ISM_USGOV_RESOURCE, for each element which 
    	specifies attribute @ism:derivativelyClassifiedBy this rule ensures that
    	attribute @ism:classificationReason or @ism:classifiedBy is NOT specified.
    </sch:p>
	  <sch:rule id="ISM-ID-00367-R1" context="*[$ISM_USGOV_RESOURCE and @ism:derivedFrom]">
	      <sch:assert test="not(@ism:classifiedBy)" flag="error" role="error">
	          [ISM-ID-00367][Error] If ISM_USGOV_RESOURCE and attribute @ism:derivedFrom is 
	          specified, then attribute @ism:classifiedBy must not be specified.
	          
	          Human Readable: USA documents that specify a derivative classifier must not also 
	          include information related to Original Classification Authorities (classificationReason and classifiedBy).
        </sch:assert>
    </sch:rule>
    
</sch:pattern>