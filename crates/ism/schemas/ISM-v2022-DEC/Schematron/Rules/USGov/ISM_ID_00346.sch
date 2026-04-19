<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="BANNER PORTION VALUECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00346">
	  <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
	  	[ISM-ID-00346][Error] If ISM_USGOV_RESOURCE and attribute @ism:nonICmarkings contains the name token [DS], 
	  	then attribute @ism:classification must have a value of [U].
	  	
	  	Human Readable: The DS (LIMDIS) nonICmarkings value in a USA document
	  	must only be used with a classification of UNCLASSIFIED.
	</sch:p>
	  <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
	  	If the document is an ISM_USGOV_RESOURCE, for each element which has 
	  	attribute @ism:nonICmarkings specified with a value containing
	  	the token [DS] this rule ensures that attribute @ism:classification is 
	  	specified with a value of [U].
	  </sch:p>
	<sch:rule id="ISM-ID-00346-R1" context="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:nonICmarkings, ('DS'))]">
		<sch:assert test="@ism:classification='U'" flag="error" role="error">
			[ISM-ID-00346][Error] If ISM_USGOV_RESOURCE and attribute @ism:nonICmarkings contains the name token [DS], 
			then attribute @ism:classification must have a value of [U].
			
			Human Readable: The DS (LIMDIS) nonICmarkings value in a USA document
			must only be used with a classification of UNCLASSIFIED.
		</sch:assert>
	</sch:rule>
</sch:pattern>