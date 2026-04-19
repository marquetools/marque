<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="BANNER PORTION TYPECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00287">
	  <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
		[ISM-ID-00287][Error] All @ism:noticeDate attributes must be of type Date. 
	</sch:p>
	  <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
	  	For all elements which contain an @ism:noticeDate attribute, this rule ensures that the noticeDate value matches the pattern
		defined for type Date. 
	</sch:p>
	  <sch:rule id="ISM-ID-00287-R1" context="*[@ism:noticeDate]">
		    <sch:assert test="util:meetsType(string(@ism:noticeDate), $DatePattern)" flag="error" role="error">
		    	[ISM-ID-00287][Error] All @ism:noticeDate attributes must be of type Date. 
		</sch:assert>
	  </sch:rule>
</sch:pattern>