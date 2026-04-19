<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="BANNER VALUECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00248">
	  <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
		[ISM-ID-00248][Error] ISM_RESOURCE_ELEMENT cannot have @ism:externalNotice set to [true].	
		
		Human Readable: ISM resource elements cannot be external notices.
	</sch:p>
	  <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
	  	If ISM_RESOURCE_ELEMENT, this rule ensures that the ISM_RESOURCE_ELEMENT does not contain @ism:externalNotice set to [true].
	</sch:p>
	  <sch:rule id="ISM-ID-00248-R1" context="*[generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT)][@ism:externalNotice]">
		    <sch:assert test="not(string(@ism:externalNotice)=string(true()))" flag="error" role="error">
			[ISM-ID-00248][Error] ISM_RESOURCE_ELEMENT cannot have @ism:externalNotice set to [true].	
			
			Human Readable: ISM resource elements cannot be external notices.
		</sch:assert>
	  </sch:rule>
</sch:pattern>