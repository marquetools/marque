<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="ROLLDOWN STRUCTURECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00239">
	  <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
		[ISM-ID-00239][Error] If ISM_USDOD_RESOURCE and attribute @ism:noticeType of
		ISM_RESOURCE_ELEMENT contains the token [DoD-Dist-A], then any element 
		which contributes to rollup should not have an attribute
		@ism:disseminationControls present.
		
		Human Readable: Distribution statement A (Public Release) is incompatible 
		with @ism:disseminationControls present for contributing portions.
	</sch:p>
	  <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
		If the document is an ISM_USDOD_RESOURCE and the attribute
		@ism:noticeType of ISM_RESOURCE_ELEMENT contains the token [DoD-Dist-A], for
		each element which specifies attribute @ism:disseminationControls 
		this rule ensures that attribute @ism:disseminationControls is not present.
	</sch:p>
	  <sch:rule id="ISM-ID-00239-R1" context="*[$ISM_USDOD_RESOURCE  and util:containsAnyOfTheTokens($ISM_RESOURCE_ELEMENT/@ism:noticeType, ('DoD-Dist-A')) and not(@ism:excludeFromRollup=true())]">
		    <sch:assert test="not(@ism:disseminationControls)" flag="error" role="error"> 
		    	[ISM-ID-00239][Error] If ISM_USDOD_RESOURCE and attribute @ism:noticeType of
		    	ISM_RESOURCE_ELEMENT contains the token [DoD-Dist-A], then any element 
		    	which contributes to rollup should not have an attribute
		    	@ism:disseminationControls present.
		    	
		    	Human Readable: Distribution statement A (Public Release) is incompatible 
		    	with @ism:disseminationControls present for contributing portions.
		</sch:assert>
	  </sch:rule>
</sch:pattern>