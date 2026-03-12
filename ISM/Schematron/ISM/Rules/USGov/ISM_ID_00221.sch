<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="BANNER PORTION STRUCTURECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00221">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00221][Error] If ISM_USGOV_RESOURCE and attribute 
        @ism:derivativelyClassifiedBy is specified, then attributes @ism:classificationReason
        or @ism:classifiedBy must not be specified.
        
        Human Readable: USA documents that are derivatively classified must not
        specify a classification reason or classified by.
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
    	If the document is an ISM_USGOV_RESOURCE, for each element which 
    	specifies attribute @ism:derivativelyClassifiedBy this rule ensures that
    	attribute @ism:classificationReason or @ism:classifiedBy is NOT specified.
    </sch:p>
	  <sch:rule id="ISM-ID-00221-R1" context="*[$ISM_USGOV_RESOURCE and @ism:derivativelyClassifiedBy]">
	      <sch:assert test="not(@ism:classificationReason or @ism:classifiedBy)" flag="error" role="error">
	          [ISM-ID-00221][Error] If ISM_USGOV_RESOURCE and attribute 
	          @ism:derivativelyClassifiedBy is specified, then attributes @ism:classificationReason
	          or @ism:classifiedBy must not be specified.
	          
	          Human Readable: USA documents that are derivatively classified must not
	          specify a classification reason or classified by.
        </sch:assert>
    </sch:rule>
    
</sch:pattern>