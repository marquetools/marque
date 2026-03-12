<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="BANNER TYPECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00272">
	  <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
		[ISM-ID-00272][Error] All @ism:compilationReason attributes must be a string with less than 1024 characters. 
	</sch:p>
	  <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
	  	For all elements which contain an @ism:compilationReason attribute, this rule ensures that the compilationReason value is a string with less
		than 1024 characters.   
	</sch:p>
	  <sch:rule id="ISM-ID-00272-R1" context="*[@ism:compilationReason]">
		    <sch:assert test="string-length(@ism:compilationReason) &lt;= 1024" flag="error" role="error">
		    	[ISM-ID-00272][Error] All @ism:compilationReason attributes must be a string with less than 1024 characters. 
		</sch:assert>
	  </sch:rule>
</sch:pattern>