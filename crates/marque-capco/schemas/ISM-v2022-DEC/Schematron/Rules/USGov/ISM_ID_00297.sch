<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="BANNER PORTION TYPECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00297">
	  <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
		[ISM-ID-00297][Error] All @ism:unregisteredNoticeType attributes must be a string with less than 2048 characters. 
	</sch:p>
	  <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
		For all elements which contain an @ism:unregisteredNoticeType attribute, this rule ensures that 
		the unregisteredNoticeType value is a string with less than 2048 characters.   
	</sch:p>
	  <sch:rule id="ISM-ID-00297-R1" context="*[@ism:unregisteredNoticeType]">
		    <sch:assert test="string-length(@ism:unregisteredNoticeType) &lt;= 2048" flag="error" role="error">
		    	[ISM-ID-00297][Error] All @ism:unregisteredNoticeType attributes must be a string with less than 2048 characters.
		</sch:assert>
	  </sch:rule>
</sch:pattern>